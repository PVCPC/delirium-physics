#include "DeliriumRagdollComponent.h"
#include "DeliriumClickToFight.h"
#include "Animation/SkeletalMeshActor.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "Engine/StaticMeshActor.h"
#include "Engine/World.h"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS

/**
 * A mannequin in an empty world, ragdolled: it stands, a 60 N s push leaves it up, a low-health shot puts it down,
 * a 200 N s one knocks it down, a leg cut keeps it down. It stands on a cube floor (read as level). DELIRIUM_TEST_MESH names the skeletal mesh (default the Third Person template's Manny).
 */
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FDeliriumRagdollTest, "Delirium.Ragdoll", EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FDeliriumRagdollTest::RunTest(const FString&)
{
	FString Path = FPlatformMisc::GetEnvironmentVariable(TEXT("DELIRIUM_TEST_MESH"));
	if (Path.IsEmpty()) Path = TEXT("/Game/Characters/Mannequins/Meshes/SKM_Manny_Simple.SKM_Manny_Simple");
	USkeletalMesh* Asset = LoadObject<USkeletalMesh>(nullptr, *Path);
	if (!TestNotNull(TEXT("mesh"), Asset)) return false;
	UWorld* W = UWorld::CreateWorld(EWorldType::Game, false);
	FWorldContext& Ctx = GEngine->CreateNewWorldContext(EWorldType::Game);
	Ctx.SetCurrentWorld(W);
	// The floor: an engine cube 20 m across, its top at 0.
	AStaticMeshActor* Floor = W->SpawnActor<AStaticMeshActor>(FVector(0, 0, -50), FRotator::ZeroRotator);
	Floor->GetStaticMeshComponent()->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cube.Cube")));
	Floor->SetActorScale3D(FVector(20, 20, 1));
	auto Run = [&](const TCHAR* What, TFunctionRef<void(UDeliriumRagdollComponent*)> Hit, float Seconds,
		TFunctionRef<void(UDeliriumRagdollComponent*)> Setup = [](UDeliriumRagdollComponent*) {}) -> FVector
	{
		ASkeletalMeshActor* A = W->SpawnActor<ASkeletalMeshActor>(FVector::ZeroVector, FRotator::ZeroRotator);
		A->GetSkeletalMeshComponent()->SetSkeletalMeshAsset(Asset);
		auto* D = NewObject<UDeliriumRagdollComponent>(A);
		D->RegisterComponent();
		Setup(D);
		FVector Head = FVector::ZeroVector;
		if (D->Ragdoll())
		{
			for (int32 F = 0; F < 60; F++) D->Step(1.f / 60.f);
			Hit(D);
			for (int32 F = 0; F < Seconds * 60; F++) D->Step(1.f / 60.f);
			Head = D->GetHips();
			D->GetUp();
			for (int32 F = 0; F < 30; F++) D->Step(1.f / 60.f);
			TestFalse(TEXT("handed back"), D->IsRagdolled());
		}
		AddInfo(FString::Printf(TEXT("%s: hips at %.0f cm"), What, Head.Z));
		A->Destroy();
		return Head;
	};
	const FVector Still = Run(TEXT("standing 5 s"), [](UDeliriumRagdollComponent*) {}, 5.f);
	TestTrue(TEXT("stands"), Still.Z > 70.f);
	const FVector Pushed = Run(TEXT("60 N s push"), [](UDeliriumRagdollComponent* D) { D->Push(FVector(60, 0, 0)); }, 5.f);
	TestTrue(TEXT("stays up after a push"), Pushed.Z > 70.f);
	const FVector Hard = Run(TEXT("200 N s push"), [](UDeliriumRagdollComponent* D) { D->Push(FVector(200, 0, 0)); }, 5.f);
	TestTrue(TEXT("down after a hard push"), Hard.Z < 40.f);
	const FVector Shot = Run(TEXT("shot, health 40"), [](UDeliriumRagdollComponent* D)
		{ D->Shoot(D->GetHips() + FVector(0, 0, 40), FVector(1, 0, 0), 40.f, TEXT("POWERFUL_WEAPON"), 40.f); }, 5.f);
	TestTrue(TEXT("down after a killing shot"), Shot.Z < 40.f);
	const FVector Cut = Run(TEXT("leg cut"), [this](UDeliriumRagdollComponent* D)
		{ TestTrue(TEXT("cut"), D->Sever(D->GetHips() + FVector(0, 10, -60), FVector(0, 0, 20))); }, 5.f);
	TestTrue(TEXT("down without a leg"), Cut.Z < 40.f);
	// The Body setting and overrides (the Playroom's bodies and Parameters tab).
	auto Nothing = [](UDeliriumRagdollComponent*) {};
	TestTrue(TEXT("Brawler stands"), Run(TEXT("Brawler"), Nothing, 5.f, [](UDeliriumRagdollComponent* D) { D->Body = TEXT("Brawler"); }).Z > 50.f);
	TestTrue(TEXT("Random stands"), Run(TEXT("Random seed 5"), Nothing, 5.f, [](UDeliriumRagdollComponent* D) { D->Body = TEXT("Random"); D->BodySeed = 5; }).Z > 50.f);
	TestTrue(TEXT("Custom stands"), Run(TEXT("Custom"), Nothing, 5.f,
		[](UDeliriumRagdollComponent* D) { D->Body = TEXT("Custom"); D->CustomBody = TEXT("1, 1, 1, 1.2, 1, 1, 95, 0"); }).Z > 50.f);
	TestTrue(TEXT("overrides"), Run(TEXT("override"), [this](UDeliriumRagdollComponent* D)
		{
			TestTrue(TEXT("good override"), D->SetOverride(TEXT("bodyBalance"), TEXT("useHeadLook"), 0.f));
			TestFalse(TEXT("bad override refused"), D->SetOverride(TEXT("bodyBalance"), TEXT("noSuchThing"), 1.f));
		}, 5.f, [](UDeliriumRagdollComponent* D) { D->Overrides = TEXT("shot.bodyStiffness=7"); }).Z > 50.f);
	GEngine->DestroyWorldContext(W);
	W->DestroyWorld(false);
	return true;
}

/** The click-to-fight example: the fourth shot kills, a punch only lands within 3 m, a blast throws only who's near. */
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FDeliriumClickToFightTest, "Delirium.ClickToFight", EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FDeliriumClickToFightTest::RunTest(const FString&)
{
	FString Path = FPlatformMisc::GetEnvironmentVariable(TEXT("DELIRIUM_TEST_MESH"));
	if (Path.IsEmpty()) Path = TEXT("/Game/Characters/Mannequins/Meshes/SKM_Manny_Simple.SKM_Manny_Simple");
	USkeletalMesh* Asset = LoadObject<USkeletalMesh>(nullptr, *Path);
	if (!TestNotNull(TEXT("mesh"), Asset)) return false;
	UWorld* W = UWorld::CreateWorld(EWorldType::Game, false);
	FWorldContext& Ctx = GEngine->CreateNewWorldContext(EWorldType::Game);
	Ctx.SetCurrentWorld(W);
	AStaticMeshActor* Floor = W->SpawnActor<AStaticMeshActor>(FVector(0, 0, -50), FRotator::ZeroRotator);
	Floor->GetStaticMeshComponent()->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cube.Cube")));
	Floor->SetActorScale3D(FVector(40, 40, 1));
	auto Man = [&](FVector At)
	{
		ASkeletalMeshActor* A = W->SpawnActor<ASkeletalMeshActor>(At, FRotator::ZeroRotator);
		A->GetSkeletalMeshComponent()->SetSkeletalMeshAsset(Asset);
		auto* D = NewObject<UDeliriumRagdollComponent>(A);
		D->RegisterComponent();
		return D;
	};
	UDeliriumRagdollComponent* A = Man(FVector::ZeroVector);
	UDeliriumRagdollComponent* B = Man(FVector(0, 800, 0));
	AActor* Player = W->SpawnActor<AActor>();
	auto* Fight = NewObject<UDeliriumClickToFight>(Player);
	Fight->RegisterComponent();
	auto Wait = [&](float S) { for (int32 F = 0; F < S * 60; F++) { A->Step(1.f / 60.f); B->Step(1.f / 60.f); } };
	const FVector Eye(-600, 0, 150), Aim(1, 0, -0.1f);
	for (int32 I = 0; I < 4; I++) { TestTrue(TEXT("shot"), Fight->Fire(Eye, Aim)); Wait(1.5f); }
	Wait(3.f);
	AddInfo(FString::Printf(TEXT("after 4 shots: hips at %.0f cm"), A->GetHips().Z));
	TestTrue(TEXT("down after four"), A->GetHips().Z < 40.f);
	TestFalse(TEXT("punch out of reach misses"), Fight->Punch(FVector(-600, 800, 150), FVector(1, 0, 0)));
	TestTrue(TEXT("punch in reach"), Fight->Punch(FVector(-200, 800, 150), FVector(1, 0, -0.2f)));
	Wait(2.f);
	TestEqual(TEXT("blast hits one"), Fight->Blast(FVector(-400, 800, 150), FVector(1, 0, -1)), 1);
	Wait(2.f);
	AddInfo(FString::Printf(TEXT("after blast: hips at %.0f cm"), B->GetHips().Z));
	TestTrue(TEXT("blast throws him down"), B->GetHips().Z < 50.f);
	GEngine->DestroyWorldContext(W);
	W->DestroyWorld(false);
	return true;
}

#endif
