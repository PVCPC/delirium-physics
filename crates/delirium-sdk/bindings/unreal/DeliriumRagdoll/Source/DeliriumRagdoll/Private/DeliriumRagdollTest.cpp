#include "DeliriumRagdollComponent.h"
#include "DeliriumClickToFight.h"
#include "Animation/SkeletalMeshActor.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "Engine/StaticMeshActor.h"
#include "Engine/World.h"
#include "Misc/AutomationTest.h"
#include "PhysicalMaterials/PhysicalMaterial.h"
#include "DeliriumSettings.h"

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
	// EnginePhysics (DELIRIUM_TEST_ENGINE=1): Chaos's bodies, the world's physics ticked after each step (its components
	// don't tick: no BeginPlay).
	const bool bEngine = FPlatformMisc::GetEnvironmentVariable(TEXT("DELIRIUM_TEST_ENGINE")) == TEXT("1");
	GetMutableDefault<UDeliriumSettings>()->bEnginePhysics = bEngine;
	// CreateWorld's worlds don't simulate.
	W->bShouldSimulatePhysics = bEngine;
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
			auto Tick = [&]() { D->Step(1.f / 60.f); if (bEngine) { W->Tick(LEVELTICK_All, 1.f / 60.f); GFrameCounter++; } };
			for (int32 F = 0; F < 60; F++) Tick();
			Hit(D);
			for (int32 F = 0; F < Seconds * 60; F++) Tick();
			Head = D->GetHips();
			D->GetUp();
			for (int32 F = 0; F < 30; F++) Tick();
			TestFalse(TEXT("handed back"), D->IsRagdolled());
		}
		AddInfo(FString::Printf(TEXT("%s: hips at %.0f cm"), What, Head.Z));
		A->Destroy();
		return Head;
	};
	const FVector Still = Run(TEXT("standing 5 s"), [](UDeliriumRagdollComponent*) {}, 5.f);
	TestTrue(TEXT("stands"), Still.Z > 70.f);
	const FVector Mapped = Run(TEXT("BoneMap"), [](UDeliriumRagdollComponent*) {}, 2.f,
		[](UDeliriumRagdollComponent* D) { D->BoneMap = TEXT("pelvis=Buttocks, NoSuchBone=Head, thigh_l=Bogus"); });
	TestTrue(TEXT("stands with a BoneMap"), Mapped.Z > 70.f);
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
	Run(TEXT("tased 2.5 s"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("tased"), D->Tase(2.5f) && D->GetState().bTased); }, 5.f);
	for (const TCHAR* B : { TEXT("Brute"), TEXT("Brawler"), TEXT("Bruiser") })
		TestTrue(FString::Printf(TEXT("%s stands"), B), Run(B, Nothing, 5.f, [B](UDeliriumRagdollComponent* D) { D->Body = B; }).Z > 50.f);
	// EnginePhysics: a 5 kg ball thrown at his back knocks him on (a mover to NM).
	if (bEngine)
	{
		const FVector Hit = Run(TEXT("ball"), [&](UDeliriumRagdollComponent* D)
			{
				AStaticMeshActor* B = W->SpawnActor<AStaticMeshActor>(D->GetHips() + FVector(0, -200, 30), FRotator::ZeroRotator);
				B->SetMobility(EComponentMobility::Movable);
				UStaticMeshComponent* M = B->GetStaticMeshComponent();
				M->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")));
				B->SetActorScale3D(FVector(0.3f));
				M->SetCollisionProfileName(UCollisionProfile::PhysicsActor_ProfileName);
				M->SetSimulatePhysics(true);
				M->SetMassOverrideInKg(NAME_None, 5.f);
				M->SetPhysicsLinearVelocity(FVector(0, 1200, 0));
			}, 2.f);
		AddInfo(FString::Printf(TEXT("ball: hips moved %.0f cm"), FVector::Dist2D(Hit, Still)));
		TestTrue(TEXT("the ball knocks him"), FVector::Dist2D(Hit, Still) > 10.f);
		// The same ball as a simulating part under a plain root (tracked by itself, not its actor).
		const FVector Kid = Run(TEXT("child ball"), [&](UDeliriumRagdollComponent* D)
			{
				AActor* B = W->SpawnActor<AActor>();
				USceneComponent* Root = NewObject<USceneComponent>(B);
				B->SetRootComponent(Root);
				Root->RegisterComponent();
				B->SetActorLocation(D->GetHips() + FVector(0, -200, 30));
				UStaticMeshComponent* M = NewObject<UStaticMeshComponent>(B);
				M->SetupAttachment(Root);
				M->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")));
				M->SetWorldScale3D(FVector(0.3f));
				M->SetCollisionProfileName(UCollisionProfile::PhysicsActor_ProfileName);
				M->RegisterComponent();
				M->SetWorldLocation(B->GetActorLocation());
				M->SetSimulatePhysics(true);
				M->SetMassOverrideInKg(NAME_None, 5.f);
				M->SetPhysicsLinearVelocity(FVector(0, 1200, 0));
			}, 2.f);
		AddInfo(FString::Printf(TEXT("child ball: hips moved %.0f cm"), FVector::Dist2D(Kid, Still)));
		TestTrue(TEXT("a child ball knocks him"), FVector::Dist2D(Kid, Still) > 10.f);
	}
	TestTrue(TEXT("Random stands"), Run(TEXT("Random seed 5"), Nothing, 5.f, [](UDeliriumRagdollComponent* D) { D->Body = TEXT("Random"); D->BodySeed = 5; }).Z > 50.f);
	TestTrue(TEXT("Custom stands"), Run(TEXT("Custom"), Nothing, 5.f,
		[](UDeliriumRagdollComponent* D) { D->Body = TEXT("Custom"); D->CustomBody = TEXT("1, 1, 1, 1.2, 1, 1, 95, 0"); }).Z > 50.f);
	TestTrue(TEXT("overrides"), Run(TEXT("override"), [this](UDeliriumRagdollComponent* D)
		{
			TestTrue(TEXT("good override"), D->SetOverride(TEXT("bodyBalance"), TEXT("useHeadLook"), 0.f));
			TestFalse(TEXT("bad override refused"), D->SetOverride(TEXT("bodyBalance"), TEXT("noSuchThing"), 1.f));
		}, 5.f, [](UDeliriumRagdollComponent* D) { D->Overrides = TEXT("shot.bodyStiffness=7"); }).Z > 50.f);
	// The Playroom's other calls (the SDK's parity calls).
	TestEqual(TEXT("weapons: the default four"), UDeliriumRagdollComponent::WeaponNames().Num(), 4);
	{
		const TArray<FDeliriumMessage> Msgs = UDeliriumRagdollComponent::Messages();
		TestTrue(FString::Printf(TEXT("messages listed (%d)"), Msgs.Num()), Msgs.Num() > 50 && Msgs[0].Params.Num() > 0);
	}
	Run(TEXT("weapon shot"), [this](UDeliriumRagdollComponent* D)
		{ TestEqual(TEXT("pistol leaves 175"), D->ShootWeapon(TEXT("pistol"), D->GetHips() + FVector(-300, 0, 40), FVector(1, 0, 0), 200.f), 175.f); }, 1.f);
	Run(TEXT("shotgun"), [this](UDeliriumRagdollComponent* D)
		{ TestTrue(TEXT("pellets strike"), D->Shotgun(D->GetHips() + FVector(-300, 0, 30), FVector(1, 0, 0)) > 0); }, 1.f);
	Run(TEXT("state reads"), [this](UDeliriumRagdollComponent* D)
		{
			const FDeliriumState S = D->GetState();
			TestTrue(TEXT("stance, contacts, awake"), S.bHasStance && S.Contacts.Num() == 21 && S.bAwake);
			FVector P, N;
			TestTrue(TEXT("raycast hits a part"), D->Raycast(D->GetHips() + FVector(-200, 0, 30), D->GetHips() + FVector(200, 0, 30), P, N) >= 0);
			float V;
			FVector Vec;
			FString T;
			TestTrue(TEXT("GetParam"), D->GetParam(TEXT("bodyBalance"), TEXT("useHeadLook"), V, Vec, T) >= 0);
			TestEqual(TEXT("CheckSetup clean"), D->CheckSetup(), FString());
			TestTrue(TEXT("Balance, Stop"), D->Balance() && D->Stop());
		}, 1.f);
	Run(TEXT("reactions"), [this](UDeliriumRagdollComponent* D)
		{
			TestTrue(TEXT("SetReactions"), D->SetReactions(int32(EDeliriumReaction::Windmill) | int32(EDeliriumReaction::Dodge) | int32(EDeliriumReaction::LookHit)));
			TestTrue(TEXT("SetDive"), D->SetDive(3.5f));
			TestTrue(TEXT("NearMiss"), D->NearMiss(D->GetHips() + FVector(-500, 30, 50), FVector(1, 0, 0)));
			TestTrue(TEXT("ThrownAt"), D->ThrownAt(D->GetHips() + FVector(200, 0, 100), FVector(-500, 0, 0)));
		}, 1.f);
	Run(TEXT("jump"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("Jump"), D->Jump(false, FVector(1, 0, 0)) && D->GetState().bJumping); }, 1.f);
	Run(TEXT("sit"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("Sit"), D->Sit() && D->GetState().bTaskRunning); }, 1.f);
	Run(TEXT("pose"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("TakePose"), D->TakePose(4.f, 6.f)); }, 1.f);
	Run(TEXT("roll up relax"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("RollUpRelax"), D->RollUpRelax()); }, 1.f);
	Run(TEXT("script"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("Script"), D->Script(500)); }, 1.f);
	Run(TEXT("muscles"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("MuscleRise"), D->MuscleRise() && D->MuscleRise(false, 0.f)); }, 1.f);
	Run(TEXT("stick"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("SetStick"), D->SetStick(true, 0.f, 1.f)); }, 1.f);
	Run(TEXT("strength"), [this](UDeliriumRagdollComponent* D) { TestTrue(TEXT("SetStrength"), D->SetStrength(1.8f) && D->SetStrength(0.f)); }, 1.f);
	Run(TEXT("attacker"), [&](UDeliriumRagdollComponent* D) { TestTrue(TEXT("SetAttacker"), D->SetAttacker(Floor, true)); D->GunReady(); }, 1.f);
	const FString Nets = FPlatformMisc::GetEnvironmentVariable(TEXT("DELIRIUM_NETS"));
	if (!Nets.IsEmpty())
	{
		Run(TEXT("learned get-up"), [&](UDeliriumRagdollComponent* D)
			{
				D->Push(FVector(200, 0, 0));
				TestTrue(TEXT("LearnedGetUp"), D->LearnedGetUp(Nets / TEXT("getup-reward.net")) && D->GetState().bLearnedGetUp);
			}, 1.f);
		Run(TEXT("learned walk"), [&](UDeliriumRagdollComponent* D)
			{ TestTrue(TEXT("LearnedWalk"), D->LearnedWalk(true, Nets / TEXT("walk.net")) && D->SetStick(true, 0.f, 1.f)); }, 1.f);
	}
	// Surfaces: the floor's physical material (ice) under a shove's feet: he slides further.
	{
		float Slid[2];
		for (int32 I = 0; I < 2; I++)
		{
			UPhysicalMaterial* Ice = nullptr;
			if (I == 1)
			{
				Ice = NewObject<UPhysicalMaterial>();
				Ice->Friction = 0.03f;
				Ice->StaticFriction = 0.03f;
			}
			Floor->GetStaticMeshComponent()->SetPhysMaterialOverride(Ice);
			FVector From = FVector::ZeroVector;
			const FVector To = Run(I ? TEXT("ice") : TEXT("concrete"), [&](UDeliriumRagdollComponent* D) { From = D->GetHips(); D->Push(FVector(120, 0, 0)); }, 2.f);
			Slid[I] = FVector::Dist2D(From, To);
		}
		Floor->GetStaticMeshComponent()->SetPhysMaterialOverride(nullptr);
		AddInfo(FString::Printf(TEXT("slid: concrete %.0f cm, ice %.0f cm"), Slid[0], Slid[1]));
		TestTrue(TEXT("ice slides further"), Slid[1] > Slid[0] + 5.f);
	}
	// A crowd (DELIRIUM_TEST_CROWD men, 10 by default) 1.5 m apart on the floor: each standing and balancing (not held up or lying)
	// after 3 s, and the real time a frame (its cost: NM's steps, Chaos's in EnginePhysics).
	{
		const FString C = FPlatformMisc::GetEnvironmentVariable(TEXT("DELIRIUM_TEST_CROWD"));
		const int32 N = C.IsEmpty() ? 10 : FCString::Atoi(*C);
		TArray<ASkeletalMeshActor*> Men;
		TArray<UDeliriumRagdollComponent*> Dolls;
		for (int32 I = 0; I < N; I++)
		{
			ASkeletalMeshActor* A = W->SpawnActor<ASkeletalMeshActor>(FVector(I % 10 * 150.f - 675.f, I / 10 * 150.f - 675.f, 0), FRotator::ZeroRotator);
			A->GetSkeletalMeshComponent()->SetSkeletalMeshAsset(Asset);
			auto* D = NewObject<UDeliriumRagdollComponent>(A);
			D->RegisterComponent();
			TestTrue(TEXT("crowd ragdolled"), D->Ragdoll());
			Men.Add(A);
			Dolls.Add(D);
		}
		auto Tick = [&]() { for (auto* D : Dolls) D->Step(1.f / 60.f); if (bEngine) W->Tick(LEVELTICK_All, 1.f / 60.f); GFrameCounter++; };
		for (int32 F = 0; F < 60; F++) Tick();
		const double T0 = FPlatformTime::Seconds();
		for (int32 F = 0; F < 120; F++) Tick();
		const double Ms = (FPlatformTime::Seconds() - T0) * 1000.0 / 120.0;
		int32 Up = 0, Balancing = 0;
		for (auto* D : Dolls) { Up += D->GetHips().Z > 70.f; Balancing += D->GetState().bBalancing; }
		AddInfo(FString::Printf(TEXT("crowd of %d: %d up, %d balancing, %.1f ms a frame"), N, Up, Balancing, Ms));
		TestTrue(TEXT("crowd stands, balancing"), Up >= N * 9 / 10 && Balancing >= N * 9 / 10);
		for (auto* A : Men) A->Destroy();
	}
	GetMutableDefault<UDeliriumSettings>()->bEnginePhysics = false;
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
