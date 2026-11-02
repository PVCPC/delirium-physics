#include "DeliriumRagdollComponent.h"
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
	auto Run = [&](const TCHAR* What, TFunctionRef<void(UDeliriumRagdollComponent*)> Hit, float Seconds) -> FVector
	{
		ASkeletalMeshActor* A = W->SpawnActor<ASkeletalMeshActor>(FVector::ZeroVector, FRotator::ZeroRotator);
		A->GetSkeletalMeshComponent()->SetSkeletalMeshAsset(Asset);
		auto* D = NewObject<UDeliriumRagdollComponent>(A);
		D->RegisterComponent();
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
	GEngine->DestroyWorldContext(W);
	W->DestroyWorld(false);
	return true;
}

#endif
