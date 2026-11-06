#include "DeliriumRagdollComponent.h"
#include "DeliriumSettings.h"
#include "delirium_physics.h"

#include "Async/Async.h"
#include "Chaos/TriangleMeshImplicitObject.h"
#include "Components/BoxComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/PoseableMeshComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/OverlapResult.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/World.h"
#include "GameFramework/Character.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "Interfaces/IPluginManager.h"
#include "Misc/Paths.h"
#include "PhysicsEngine/BodySetup.h"
#include "Physics/PhysicsInterfaceCore.h"

DEFINE_LOG_CATEGORY_STATIC(LogDelirium, Log, All);

namespace
{
	NmBuild* BaseBuild = nullptr;
	/** Each kind of character's tuned build (null while it's being tuned). */
	TMap<FString, NmBuild*> Tuned;
	FCriticalSection TunedLock;

	/** Worlds ragdolls share, so they push and pile on each other: one per spot (same size, ground within 50 cm,
	 *  within 5 m of where it was made, its level read there), freed with its last ragdoll. */
	struct FShared { NmWorld* World; float Scale; FVector At; int32 Refs; };
	TArray<FShared> Shared;

	/** RegisterCar's: the actor, its box (centre in its frame, half size, cm), its place and speed (once a frame). */
	struct FCar { TWeakObjectPtr<AActor> Actor; FVector Centre, Half, Last, Vel; uint64 Frame; bool bPlayer; EDeliriumMover Kind; bool bBrace, bKnockDown; };
	TArray<FCar> CarList;

	FString LastError()
	{
		char Buf[512] = {};
		nm_last_error(Buf, sizeof Buf);
		return UTF8_TO_TCHAR(Buf);
	}

	/** The Body setting, for the character and its background tune alike. */
	struct FBody
	{
		FString Name;
		int32 Seed;
		float Variance;
		bool bSymmetric;
		FString Custom;

		/** The fitted build `B` reshaped (a new build); null: Fitted, keep it, or failed (`Error` set). */
		NmBuild* Make(NmBuild* B, FString& Error) const
		{
			NmBuild* R;
			if (Name.IsEmpty() || Name == TEXT("Fitted")) return nullptr;
			if (Name == TEXT("Random")) R = nm_build_random(B, Seed, Variance, bSymmetric);
			else if (Name == TEXT("Custom"))
			{
				TArray<FString> P;
				Custom.ParseIntoArray(P, TEXT(","));
				if (P.Num() != 8)
				{
					Error = TEXT("CustomBody needs 8 numbers");
					return nullptr;
				}
				float V[8];
				for (int32 I = 0; I < 8; I++) V[I] = FCString::Atof(*P[I].TrimStartAndEnd());
				R = nm_build_sized(B, V[0], V[1], V[2], V[3], V[4], V[5], V[6], V[7]);
			}
			else R = nm_build_named(B, TCHAR_TO_UTF8(*Name));
			if (!R) Error = LastError();
			return R;
		}

		/** Kinds tuned apart: the body's own. */
		FString Key() const
		{
			if (Name.IsEmpty() || Name == TEXT("Fitted")) return FString();
			if (Name == TEXT("Random")) return FString::Printf(TEXT(" Random %d %g %d"), Seed, Variance, bSymmetric);
			if (Name == TEXT("Custom")) return TEXT(" Custom ") + Custom;
			return TEXT(" ") + Name;
		}
	};

	/** "behaviour.param=value, ...": each one (bad ones in the log). */
	TArray<TTuple<FString, FString, float>> ParseOverrides(const FString& S)
	{
		TArray<TTuple<FString, FString, float>> Out;
		TArray<FString> Items;
		S.ParseIntoArray(Items, TEXT(","));
		for (const FString& Item : Items)
		{
			FString K, V, M, P;
			if (Item.TrimStartAndEnd().IsEmpty()) continue;
			if (!Item.Split(TEXT("="), &K, &V) || !K.TrimStartAndEnd().Split(TEXT("."), &M, &P) || !V.TrimStartAndEnd().IsNumeric())
			{
				UE_LOG(LogDelirium, Warning, TEXT("Overrides: \"%s\" isn't behaviour.param=value"), *Item);
				continue;
			}
			Out.Add({ M, P, FCString::Atof(*V.TrimStartAndEnd()) });
		}
		return Out;
	}

	bool Check(int32 Result, const TCHAR* What)
	{
		if (Result < 0) UE_LOG(LogDelirium, Warning, TEXT("%s: %s"), What, *LastError());
		return Result >= 0;
	}

	using FSaid = TArray<TTuple<FString, int32, float>>;
	/** The ragdolls in NM's own physics, stepped a world's at once (over the machine's cores): each one's world, the
	 *  steps it was given ahead of its own clock, and what its NM said in them. */
	struct FLead { NmWorld* World; int32 Ahead; FSaid Said; };
	TMap<NmAgent*, FLead> Lead;

	/** One 1/60 s step for `Agent`: the first of its world to need it steps them all; the rest take theirs. */
	void LeadStep(NmAgent* Agent, NmWorld* World, FSaid& Said)
	{
		FLead& Me = Lead.FindOrAdd(Agent, { World, 0, {} });
		if (Me.Ahead == 0)
		{
			TArray<NmAgent*> These;
			for (const auto& L : Lead) if (L.Value.World == World) These.Add(L.Key);
			Check(nm_world_step_agents(World, These.GetData(), These.Num(), 1.f / 60.f, 0), TEXT("nm_world_step_agents"));
			for (NmAgent* A : These)
			{
				FLead& L = Lead[A];
				// A ragdoll whose clock stopped (its component not ticking) isn't owed more than a hitch's steps.
				L.Ahead = FMath::Min(L.Ahead + 1, 4);
				for (int32 I = 0, N = nm_agent_feedback_count(A); I < N; I++)
				{
					char Name[64] = {};
					int32 Slot = 0;
					float Value = 0.f;
					if (nm_agent_feedback_get(A, I, Name, sizeof(Name), &Slot, &Value) >= 0) L.Said.Add({ FString(ANSI_TO_TCHAR(Name)), Slot, Value });
				}
			}
		}
		FLead& Mine = Lead[Agent];
		Mine.Ahead--;
		Said.Append(MoveTemp(Mine.Said));
		Mine.Said.Reset();
	}

	// NM is metres, Z up, right-handed; Unreal centimetres, Z up, left-handed: y flips, a rotation (x, y, z, w) becomes
	// (-x, y, -z, w), both ways. Impulses are N s in both (only y flips).
	void ToNm(const FVector& V, float* O) { O[0] = V.X / 100.f; O[1] = -V.Y / 100.f; O[2] = V.Z / 100.f; }
	FVector FromNm(const float* F) { return FVector(F[0] * 100.f, -F[1] * 100.f, F[2] * 100.f); }
	void ToNmDir(const FVector& V, float* O) { O[0] = V.X; O[1] = -V.Y; O[2] = V.Z; }
	void ToNmTurn(const FQuat& Q, float* O) { O[0] = -Q.X; O[1] = Q.Y; O[2] = -Q.Z; O[3] = Q.W; }
	FQuat FromNmTurn(const float* F) { return FQuat(-F[0], F[1], -F[2], F[3]); }
	// EnginePhysics: a part's body is the part turned so its Y (NM's capsules' axis) is the body's Z (Unreal's).
	const FQuat PartToBody(FVector::XAxisVector, -UE_HALF_PI);

	FString Resolve(const FString& P)
	{
		return P.IsEmpty() || !FPaths::IsRelative(P) ? P : FPaths::ConvertRelativePathToFull(FPaths::ProjectDir() / P);
	}

	/** The level's static collision within `Radius` of `At` (boxes, convex hulls, triangle meshes), into the world.
	 *  ponytail: spheres, capsules and landscapes left out (the floor under the feet covers flat ground); read once. */
	int32 Level(UWorld* W, const AActor* Owner, const FVector& At, float Radius, NmWorld* World)
	{
		TArray<FOverlapResult> Hits;
		FCollisionQueryParams Q(TEXT("DeliriumLevel"), false, Owner);
		W->OverlapMultiByObjectType(Hits, At, FQuat::Identity, FCollisionObjectQueryParams(ECC_WorldStatic), FCollisionShape::MakeSphere(Radius), Q);
		TArray<float> Pts;
		TArray<uint32> Tris;
		auto Vert = [&](const FVector& V) { const int32 I = Pts.Num(); Pts.AddUninitialized(3); ToNm(V, &Pts[I]); return uint32(I / 3); };
		for (const FOverlapResult& H : Hits)
		{
			UPrimitiveComponent* C = H.GetComponent();
			UBodySetup* B = C ? C->GetBodySetup() : nullptr;
			if (!B) continue;
			const FTransform T = C->GetComponentTransform();
			for (const FKBoxElem& Box : B->AggGeom.BoxElems)
			{
				const FTransform BT = Box.GetTransform() * T;
				uint32 K[8];
				for (int32 I = 0; I < 8; I++)
					K[I] = Vert(BT.TransformPosition(FVector((I & 1 ? 0.5 : -0.5) * Box.X, (I & 2 ? 0.5 : -0.5) * Box.Y, (I & 4 ? 0.5 : -0.5) * Box.Z)));
				static const int32 F[6][4] = { {0, 2, 3, 1}, {4, 5, 7, 6}, {0, 1, 5, 4}, {2, 6, 7, 3}, {0, 4, 6, 2}, {1, 3, 7, 5} };
				for (const auto& Q4 : F)
					Tris.Append({ K[Q4[0]], K[Q4[1]], K[Q4[2]], K[Q4[0]], K[Q4[2]], K[Q4[3]] });
			}
			for (const FKConvexElem& Cx : B->AggGeom.ConvexElems)
			{
				const FTransform CT = Cx.GetTransform() * T;
				const uint32 First = Pts.Num() / 3;
				for (const FVector& V : Cx.VertexData) Vert(CT.TransformPosition(V));
				for (int32 I : Cx.IndexData) Tris.Add(First + I);
			}
			for (const Chaos::FTriangleMeshImplicitObjectPtr& M : B->TriMeshGeometries)
			{
				if (!M) continue;
				const uint32 First = Pts.Num() / 3;
				const auto& P = M->Particles();
				for (uint32 I = 0; I < P.Size(); I++) Vert(T.TransformPosition(FVector(P.GetX(I))));
				const Chaos::FTrimeshIndexBuffer& E = M->Elements();
				auto Add = [&](const auto& Buf) { for (const auto& Tri : Buf) Tris.Append({ First + Tri[0], First + Tri[1], First + Tri[2] }); };
				if (E.RequiresLargeIndices()) Add(E.GetLargeIndexBuffer()); else Add(E.GetSmallIndexBuffer());
			}
		}
		if (Tris.Num() == 0) return 0;
		// The y flip mirrors them: turn each back to face the same way.
		for (int32 I = 0; I < Tris.Num(); I += 3) Swap(Tris[I + 1], Tris[I + 2]);
		Check(nm_world_add_mesh(World, Pts.GetData(), Pts.Num() / 3, Tris.GetData(), Tris.Num() / 3, nullptr), TEXT("nm_world_add_mesh"));
		return Tris.Num() / 3;
	}
}

UDeliriumRagdollComponent::UDeliriumRagdollComponent()
{
	PrimaryComponentTick.bCanEverTick = true;
	PrimaryComponentTick.TickGroup = TG_PostPhysics;
}

bool UDeliriumRagdollComponent::Start()
{
	static int32 State = 0; // 0 not tried, 1 ready, -1 failed
	if (State) return State > 0;
	State = -1;
	const UDeliriumSettings* S = GetDefault<UDeliriumSettings>();
	// The SDK sits next to the plugin's module.
	TSharedPtr<IPlugin> Me = IPluginManager::Get().FindPlugin(TEXT("DeliriumRagdoll"));
	const FString Dll = Me ? FPaths::Combine(Me->GetBaseDir(), TEXT("Binaries/Win64/delirium_physics.dll")) : FString();
	if (!Dll.IsEmpty() && !FPlatformProcess::GetDllHandle(*Dll))
	{
		UE_LOG(LogDelirium, Error, TEXT("can't load %s"), *Dll);
		return false;
	}
	if (!S->GameExe.IsEmpty() && !Check(nm_unlock(TCHAR_TO_UTF8(*Resolve(S->GameExe))), TEXT("GameExe"))) return false;
	if (!Check(nm_init(TCHAR_TO_UTF8(*Resolve(S->Character))), TEXT("Character"))) return false;
	if (!S->TaskParams.IsEmpty() && !Check(nm_load_task_params(TCHAR_TO_UTF8(*Resolve(S->TaskParams))), TEXT("TaskParams"))) return false;
	nm_tune_file(TCHAR_TO_UTF8(*FPaths::ConvertRelativePathToFull(FPaths::ProjectSavedDir() / TEXT("delirium-tunes.txt"))));
	BaseBuild = nm_build_load(TCHAR_TO_UTF8(*Resolve(S->Build)));
	if (!BaseBuild)
	{
		UE_LOG(LogDelirium, Error, TEXT("Build: %s"), *LastError());
		return false;
	}
	nm_set_headshot_kills(S->bHeadshotKills ? 1 : 0);
	State = 1;
	return true;
}

float UDeliriumRagdollComponent::Ground(const FVector& Hips) const
{
	FHitResult Hit;
	FCollisionQueryParams Q(TEXT("DeliriumGround"), false, GetOwner());
	if (GetWorld()->LineTraceSingleByObjectType(Hit, Hips, Hips - FVector(0, 0, 500), FCollisionObjectQueryParams(ECC_WorldStatic), Q))
		return Hit.ImpactPoint.Z;
	// No floor below: its lowest bone.
	float Low = Hips.Z;
	for (int32 B : Bones) Low = FMath::Min(Low, Mesh->GetBoneTransform(B).GetLocation().Z);
	return Low;
}

bool UDeliriumRagdollComponent::Take()
{
	if (Agent) return true;
	if (!Start()) return false;
	if (!Mesh) Mesh = GetOwner()->FindComponentByClass<USkeletalMeshComponent>();
	USkeletalMesh* Asset = Mesh ? Mesh->GetSkeletalMeshAsset() : nullptr;
	if (!Asset)
	{
		UE_LOG(LogDelirium, Warning, TEXT("%s: no skeletal mesh"), *GetOwner()->GetName());
		return false;
	}
	const FReferenceSkeleton& Ref = Asset->GetRefSkeleton();
	const int32 All = Ref.GetNum();
	TArray<int32> Guess;
	for (int32 B = 0; B < All; B++) Guess.Add(nm_rig_part_for_name(TCHAR_TO_UTF8(*Ref.GetBoneName(B).ToString())));
	// The hips: the bone named so, else the thighs' shared parent.
	int32 Hips = Guess.IndexOfByKey(0);
	if (Hips < 0)
	{
		const int32 L = Guess.IndexOfByKey(1), R = Guess.IndexOfByKey(4);
		Hips = L >= 0 && R >= 0 && Ref.GetParentIndex(L) == Ref.GetParentIndex(R) ? Ref.GetParentIndex(L) : -1;
	}
	if (Hips < 0)
	{
		UE_LOG(LogDelirium, Warning, TEXT("%s: no hips among its bones"), *GetOwner()->GetName());
		return false;
	}
	// The hips and everything under them, parents first (the reference skeleton's own order).
	TArray<int32> Local;
	Local.Init(-1, All);
	Bones.Reset();
	for (int32 B = Hips; B < All; B++)
		if (B == Hips || (Ref.GetParentIndex(B) >= 0 && Local[Ref.GetParentIndex(B)] >= 0)) { Local[B] = Bones.Num(); Bones.Add(B); }
	const int32 N = Bones.Num();
	// The skeleton in its bind pose (not the pose it's in now: a body fitted to a mid-stride pose comes out bent
	// against its own joint limits), placed where the mesh is.
	TArray<FTransform> Cs;
	Cs.SetNum(All);
	for (int32 B = 0; B < All; B++)
		Cs[B] = Ref.GetRefBonePose()[B] * (Ref.GetParentIndex(B) >= 0 ? Cs[Ref.GetParentIndex(B)] : FTransform::Identity);
	const FTransform ToWorld = Mesh->GetComponentTransform();
	TArray<int32> Parents, Parts;
	TArray<float> Places, Rest;
	Places.SetNum(N * 3);
	Rest.SetNum(N * 7);
	for (int32 I = 0; I < N; I++)
	{
		const int32 B = Bones[I];
		Parents.Add(I == 0 ? -1 : Local[Ref.GetParentIndex(B)]);
		Parts.Add(I == 0 ? 0 : Guess[B]);
		const FTransform W = Cs[B] * ToWorld;
		ToNm(W.GetLocation(), &Places[I * 3]);
		ToNm(W.GetLocation(), &Rest[I * 7]);
		ToNmTurn(W.GetRotation(), &Rest[I * 7 + 3]);
	}
	char Report[2048] = {};
	if (nm_rig_check(N, Parents.GetData(), Places.GetData(), Parts.GetData(), Report, sizeof Report) > 0)
		UE_LOG(LogDelirium, Warning, TEXT("%s's skeleton: %s"), *Asset->GetName(), UTF8_TO_TCHAR(Report));
	float Joints[63];
	if (!Check(nm_rig_joints(N, Parents.GetData(), Places.GetData(), Parts.GetData(), Joints), TEXT("nm_rig_joints"))) return false;
	const UDeliriumSettings* S = GetDefault<UDeliriumSettings>();
	const int32 Mode = S->Size == TEXT("Homer") ? NM_SIZE_BASE : S->Size == TEXT("Real") ? NM_SIZE_REAL : NM_SIZE_AUTO;
	Build = nm_build_fit_sized(BaseBuild, Joints, Mode);
	if (!Build)
	{
		UE_LOG(LogDelirium, Warning, TEXT("%s: %s"), *GetOwner()->GetName(), *LastError());
		return false;
	}
	Scale = nm_build_get_scale(Build);
	const FBody Shape{ Body, BodySeed ? BodySeed : (int32)FPlatformTime::Cycles(), BodyVariance, bBodySymmetric, CustomBody };
	FString BodyError;
	if (NmBuild* R = Shape.Make(Build, BodyError))
	{
		nm_build_destroy(Build);
		Build = R;
	}
	else if (!BodyError.IsEmpty()) UE_LOG(LogDelirium, Warning, TEXT("%s body %s: %s"), *GetOwner()->GetName(), *Body, *BodyError);
	// Each kind's balance settings, searched once in the background and copied to the rest of its kind.
	if (S->bAdaptive)
	{
		FScopeLock Lock(&TunedLock);
		const FString Kind = Asset->GetPathName() + Shape.Key();
		if (NmBuild** T = Tuned.Find(Kind)) { if (*T) nm_build_copy_tune(Build, *T); }
		else
		{
			Tuned.Add(Kind, nullptr);
			TArray<float> J(Joints, 63);
			AsyncTask(ENamedThreads::AnyBackgroundThreadNormalTask, [Kind, J, Mode, Shape]
			{
				NmBuild* T = nm_build_fit_sized(BaseBuild, J.GetData(), Mode);
				FString Ignored;
				if (NmBuild* R = T ? Shape.Make(T, Ignored) : nullptr)
				{
					nm_build_destroy(T);
					T = R;
				}
				const int32 Got = T ? nm_build_tune(T, 200) : -1;
				FScopeLock Lock(&TunedLock);
				Tuned.Add(Kind, T);
				UE_LOG(LogDelirium, Log, TEXT("%s: balance tuned, stands through %d of 3 pushes"), *Kind, Got);
			});
		}
	}
	const FVector HipsAt = (Cs[Hips] * ToWorld).GetLocation();
	const FVector Now = Mesh->GetBoneTransform(Hips).GetLocation();
	const float Floor = Ground(Now);
	const FVector Spot(Now.X, Now.Y, Floor);
	const int32 Near = Shared.IndexOfByPredicate([&](const FShared& W)
		{ return W.Scale == Scale && FMath::Abs(W.At.Z - Floor) < 50.f * Scale && FVector::Dist(W.At, Spot) < 500.f * Scale; });
	if (Near >= 0)
	{
		World = Shared[Near].World;
		Shared[Near].Refs++;
	}
	else
	{
		World = nm_world_create();
		Check(nm_world_set_scale(World, Scale), TEXT("nm_world_set_scale"));
		Shared.Add({ World, Scale, Spot, 1 });
		const int32 Tris = Level(GetWorld(), GetOwner(), Now, 1500.f, World);
		UE_LOG(LogDelirium, Log, TEXT("%s: %d level triangles"), *GetOwner()->GetName(), Tris);
		float F[3];
		ToNm(Spot, F);
		Check(nm_world_add_floor(World, F[0], F[1], F[2], 100.f * Scale), TEXT("nm_world_add_floor"));
	}
	float At[3];
	ToNm(FVector(HipsAt.X, HipsAt.Y, Floor), At);
	Agent = nm_agent_create(Build, At, 0.f, FMath::FRand());
	if (!Agent)
	{
		UE_LOG(LogDelirium, Warning, TEXT("%s: %s"), *GetOwner()->GetName(), *LastError());
		Release();
		return false;
	}
	if (S->Strength != 1.f) Check(nm_agent_set_strength(Agent, S->Strength), TEXT("Strength"));
	// Arms out early: from 1.3 m (the game's 0.65), predicted 0.45 s ahead (its car set's), every brace it makes.
	if (S->bArmsOutEarly)
	{
		Check(nm_agent_set_override(Agent, "braceForImpact", "braceDistance", 1.3f), TEXT("ArmsOutEarly"));
		Check(nm_agent_set_override(Agent, "braceForImpact", "targetPredictionTime", 0.45f), TEXT("ArmsOutEarly"));
	}
	TArray<TTuple<FString, FString, float>> Overridden = ParseOverrides(Overrides);
	Overridden.Append(Added);
	for (const auto& O : Overridden)
		Check(nm_agent_set_override(Agent, TCHAR_TO_UTF8(*O.Get<0>()), TCHAR_TO_UTF8(*O.Get<1>()), O.Get<2>()), TEXT("Overrides"));
	Rig = nm_rig_create(Agent, N, Parents.GetData(), Rest.GetData(), Parts.GetData());
	if (!Rig)
	{
		UE_LOG(LogDelirium, Warning, TEXT("%s: %s"), *GetOwner()->GetName(), *LastError());
		Release();
		return false;
	}
	// It starts in the pose its animation has it in now, moving as the character moved: no snap.
	TArray<float> Anim;
	Anim.SetNum(N * 7);
	for (int32 I = 0; I < N; I++)
	{
		const FTransform W = Mesh->GetBoneTransform(Bones[I]);
		ToNm(W.GetLocation(), &Anim[I * 7]);
		ToNmTurn(W.GetRotation(), &Anim[I * 7 + 3]);
	}
	Check(nm_rig_take(Rig, Agent, Anim.GetData(), nullptr, 0.f), TEXT("nm_rig_take"));
	if (S->bRecentreJoints) nm_agent_recentre_joint(Agent, -1);
	TArray<FString> Holds;
	S->Hold.ParseIntoArray(Holds, TEXT(","));
	for (const FString& Item : Holds)
	{
		FString Mask, Values;
		TArray<FString> V;
		if (Item.Split(TEXT("="), &Mask, &Values) && Values.ParseIntoArray(V, TEXT(":")) == 3)
			Check(nm_agent_hold(Agent, TCHAR_TO_UTF8(*Mask.TrimStartAndEnd()), FCString::Atof(*V[0]), FCString::Atof(*V[1]), FCString::Atof(*V[2])), TEXT("Hold"));
		else
			UE_LOG(LogDelirium, Warning, TEXT("Hold: \"%s\" isn't mask=stiffness:damping:gravity"), *Item.TrimStartAndEnd());
	}
	TArray<FString> Follows;
	S->Follow.ParseIntoArray(Follows, TEXT(","));
	bFollow = false;
	for (const FString& Item : Follows)
	{
		FString Mask, Weight;
		if (Item.Split(TEXT("="), &Mask, &Weight) && Weight.TrimStartAndEnd().IsNumeric())
			bFollow |= Check(nm_agent_follow(Agent, TCHAR_TO_UTF8(*Mask.TrimStartAndEnd()), FCString::Atof(*Weight)), TEXT("Follow"));
		else
			UE_LOG(LogDelirium, Warning, TEXT("Follow: \"%s\" isn't mask=weight"), *Item.TrimStartAndEnd());
	}
	const FVector Vel = GetOwner()->GetVelocity();
	if (!Vel.IsNearlyZero())
	{
		float V[3];
		ToNmDir(Vel / 100.f, V);
		Check(nm_agent_add_velocity(Agent, V), TEXT("nm_agent_add_velocity"));
	}
	if (S->bEnginePhysics) BuildParts();
	PoseOut.SetNum(N * 4 + 3);
	// A posed copy of the mesh shows the body; the game's own mesh, animation and movement wait.
	if (Posed) { Posed->DestroyComponent(); Posed = nullptr; }
	Blend = 0.f;
	Posed = NewObject<UPoseableMeshComponent>(GetOwner());
	Posed->SetSkinnedAssetAndUpdate(Asset);
	for (int32 M = 0; M < Mesh->GetNumMaterials(); M++) Posed->SetMaterial(M, Mesh->GetMaterial(M));
	Posed->SetupAttachment(Mesh->GetAttachParent());
	Posed->SetRelativeTransform(Mesh->GetRelativeTransform());
	Posed->RegisterComponent();
	Posed->CopyPoseFromSkeletalComponent(Mesh);
	Mesh->SetVisibility(false, true);
	// Follow: the hidden mesh animates on for its joints to aim at (Step); else it waits.
	if (bFollow)
	{
		FollowTick = Mesh->VisibilityBasedAnimTickOption;
		Mesh->VisibilityBasedAnimTickOption = EVisibilityBasedAnimTickOption::AlwaysTickPoseAndRefreshBones;
		Incoming.SetNum(N * 7);
	}
	else Mesh->bPauseAnims = true;
	if (ACharacter* C = Cast<ACharacter>(GetOwner()))
	{
		C->GetCharacterMovement()->StopMovementImmediately();
		C->GetCharacterMovement()->DisableMovement();
	}
	Pinned = GetOwner()->GetActorTransform();
	if (S->bBalance)
	{
		nm_msg_begin(Agent, "bodyBalance");
		nm_msg_set_bool(Agent, "start", 1);
		Check(nm_msg_send(Agent, World), TEXT("bodyBalance"));
	}
	bTold = false;
	Clock = 0.f;
	UE_LOG(LogDelirium, Log, TEXT("ragdolled %s (%d bones, scale %.2f)"), *GetOwner()->GetName(), N, Scale);
	return true;
}

void UDeliriumRagdollComponent::Pose()
{
	GetOwner()->SetActorTransform(Pinned);
	const int32 HipsOut = nm_rig_pose(Rig, Agent, PoseOut.GetData());
	const int32 N = Bones.Num();
	TArray<FTransform>& Local = Posed->BoneSpaceTransforms;
	// The hips' turn (and place) is a world one, every other bone's is under its parent.
	for (int32 I = 1; I < N; I++) Local[Bones[I]].SetRotation(FromNmTurn(&PoseOut[I * 4]));
	FTransform HipsWorld(FromNmTurn(&PoseOut[0]), HipsOut == 0 ? FromNm(&PoseOut[N * 4]) : Posed->GetBoneTransform(Bones[0]).GetLocation());
	const int32 Parent = Posed->GetSkinnedAsset()->GetRefSkeleton().GetParentIndex(Bones[0]);
	const FTransform ParentWorld = Parent >= 0 ? Posed->GetBoneTransform(Parent) : Posed->GetComponentTransform();
	FTransform L = HipsWorld.GetRelativeTransform(ParentWorld);
	L.SetScale3D(Local[Bones[0]].GetScale3D());
	Local[Bones[0]] = L;
	Posed->RefreshBoneTransforms();
}

void UDeliriumRagdollComponent::Step(float Seconds)
{
	if (Blend > 0.f && Posed && Mesh)
	{
		// Getting up: from the ragdoll's last pose into the animation's, then the animated mesh again.
		Blend = FMath::Max(Blend - Seconds, 0.f);
		const float T = 1.f - Blend / FMath::Max(GetUpBlend, 1e-3f);
		const TArray<FTransform>& Anim = Mesh->GetBoneSpaceTransforms();
		TArray<FTransform>& Local = Posed->BoneSpaceTransforms;
		for (int32 B : Bones)
			if (Anim.IsValidIndex(B)) Local[B].Blend(Last[B], Anim[B], T);
		Posed->RefreshBoneTransforms();
		if (Blend <= 0.f)
		{
			Mesh->SetVisibility(true, true);
			Mesh->VisibilityBasedAnimTickOption = WasTick;
			Posed->DestroyComponent();
			Posed = nullptr;
		}
		return;
	}
	const int32 NCars = Cars();
	if (!Agent) return;
	if (bFollow && Mesh)
	{
		for (int32 I = 0; I < Bones.Num(); I++)
		{
			const FTransform W = Mesh->GetBoneTransform(Bones[I]);
			ToNm(W.GetLocation(), &Incoming[I * 7]);
			ToNmTurn(W.GetRotation(), &Incoming[I * 7 + 3]);
		}
		Check(nm_rig_incoming(Rig, Agent, Incoming.GetData()), TEXT("nm_rig_incoming"));
	}
	Clock = FMath::Min(Clock + Seconds, 4.f / 60.f);
	FSaid Said;
	// EnginePhysics: every quarter second, loose physics objects near him to NM as movers.
	if (PartBodies.Num() > 0 && (Look -= Seconds) <= 0.f)
	{
		Look = 0.25f;
		MoversNear();
	}
	for (; Clock >= 1.f / 60.f; Clock -= 1.f / 60.f) LeadStep(Agent, World, Said);
	for (const auto& F : Said) OnFeedback.Broadcast(F.Get<0>(), F.Get<1>(), F.Get<2>());
	if (!Agent) return;
	const bool bNowDown = nm_agent_has_fallen(Agent) == 1;
	if (bNowDown && !bDown) OnFell.Broadcast();
	bDown = bNowDown;
	// A car hit gathered over the steps: it reacts as the game picks (brace, roll up, stagger, limp).
	if (NCars > 0)
	{
		const FVector Here = GetOwner()->GetActorLocation();
		const FCar* Near = nullptr;
		for (const FCar& C : CarList)
			if (!Near || FVector::DistSquared(C.Last, Here) < FVector::DistSquared(Near->Last, Here)) Near = &C;
		float Damage = 0.f;
		int32 Box = -1;
		if (nm_agent_car_hit(Agent, World, HitHealth, HitAgility, Near && Near->bPlayer ? 1 : 0, &Damage, &Box) == 1)
		{
			Bracing = nullptr;
			OnCarHit.Broadcast(Damage, CarList.IsValidIndex(Box) ? CarList[Box].Actor.Get() : nullptr);
		}
	}
	// EnginePhysics: the bodies to where NM has the parts, for Chaos's next step (moved, so they push what's in the way).
	for (int32 K = 0; K < PartBodies.Num(); K++)
	{
		float P[3], Q[4];
		nm_agent_get_part_transform(Agent, K, P, Q);
		PartBodies[K]->SetWorldTransform(FTransform(FromNmTurn(Q) * PartToBody, FromNm(P)));
	}
	Pose();
	float Head[3];
	int32 Up = 0;
	const float Still = nm_agent_get_lying(Agent, Head, &Up);
	if (Still < 0.5f) { bTold = false; return; }
	if (!bTold)
	{
		bTold = true;
		const FVector Facing = FVector(Head[0], -Head[1], 0.f).GetSafeNormal() * (Up ? -1.f : 1.f);
		OnLyingStill.Broadcast(Up == 1, Facing);
	}
	if (GetUpAfter > 0.f && Agent && nm_agent_should_get_up(Agent, GetUpAfter) == 1) GetUp();
}

void UDeliriumRagdollComponent::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
	Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
	Step(DeltaTime);
}

FVector UDeliriumRagdollComponent::GetHips() const
{
	return Posed && Bones.Num() ? Posed->GetBoneTransform(Bones[0]).GetLocation() : FVector::ZeroVector;
}

bool UDeliriumRagdollComponent::Ragdoll() { return Take(); }

TArray<FString> UDeliriumRagdollComponent::Bodies()
{
	return { TEXT("Fitted"), TEXT("Small"), TEXT("Average"), TEXT("Large"), TEXT("Lanky"), TEXT("Fat"), TEXT("Stocky"),
		TEXT("Waddler"), TEXT("Guard"), TEXT("Brute"), TEXT("Brawler"), TEXT("Bruiser"), TEXT("Random"), TEXT("Custom") };
}

bool UDeliriumRagdollComponent::SetOverride(FString Behaviour, FString Param, float Value)
{
	if (Agent && !Check(nm_agent_set_override(Agent, TCHAR_TO_UTF8(*Behaviour), TCHAR_TO_UTF8(*Param), Value), TEXT("SetOverride")))
		return false;
	Added.RemoveAll([&](const TTuple<FString, FString, float>& O) { return O.Get<0>() == Behaviour && O.Get<1>() == Param; });
	Added.Add({ Behaviour, Param, Value });
	return true;
}

void UDeliriumRagdollComponent::ClearOverrides()
{
	Added.Reset();
	Overrides.Reset();
	if (Agent) nm_agent_clear_overrides(Agent);
}

bool UDeliriumRagdollComponent::Shoot(FVector Point, FVector Direction, float Force, FString Set, float Health)
{
	if (!Take()) return false;
	float At[3], Dir[3], Imp[3];
	ToNm(Point, At);
	ToNmDir(Direction.GetSafeNormal(), Dir);
	ToNmDir(Direction.GetSafeNormal() * Force * GetDefault<UDeliriumSettings>()->ShotForce, Imp);
	const int32 Part = FMath::Max(nm_agent_nearest_part(Agent, At), 0);
	Check(nm_agent_apply_push(Agent, At, Imp), TEXT("Shoot"));
	return Check(nm_agent_task_shot(Agent, World, TCHAR_TO_UTF8(*Set), Part, At, Dir, Health), TEXT("Shoot"));
}

bool UDeliriumRagdollComponent::ShootAlong(FVector From, FVector Aim, float Force, FString Set, float Health)
{
	if (!Take()) return false;
	float F[3], D[3], Hit[3];
	ToNm(From, F);
	ToNmDir(Aim.GetSafeNormal(), D);
	if (nm_agent_aim_hit(Agent, F, D, Hit) < 0) return false;
	return Shoot(FromNm(Hit), Aim, Force, Set, Health);
}

bool UDeliriumRagdollComponent::Push(FVector Impulse)
{
	if (!Take()) return false;
	float At[3], Imp[3];
	ToNm(GetHips() + FVector(0, 0, 35.f * Scale), At);
	ToNmDir(Impulse, Imp);
	return Check(nm_agent_apply_push(Agent, At, Imp), TEXT("Push"));
}

bool UDeliriumRagdollComponent::Explode(FVector At, float Radius, float Speed)
{
	if (!Take()) return false;
	float A[3];
	ToNm(At, A);
	return Check(nm_agent_task_explosion(Agent, World, A, Radius / 100.f, Speed * GetDefault<UDeliriumSettings>()->ExplosionForce), TEXT("Explode"));
}

bool UDeliriumRagdollComponent::MeleeHit(FVector Point, FVector Impulse, FVector AttackerHead, bool bTimid)
{
	if (!Take()) return false;
	float P[3], I[3], H[3];
	ToNm(Point, P);
	ToNmDir(Impulse * GetDefault<UDeliriumSettings>()->MeleeForce, I);
	ToNm(AttackerHead, H);
	return Check(nm_agent_task_melee_hit(Agent, World, P, I, H, bTimid ? 1 : 0), TEXT("MeleeHit"));
}

bool UDeliriumRagdollComponent::OnFire(bool bBurning, float Health)
{
	if (!Take()) return false;
	return Check(bBurning ? nm_agent_task_on_fire(Agent, World, Health) : nm_agent_set_on_fire(Agent, 0, Health), TEXT("OnFire"));
}

bool UDeliriumRagdollComponent::Tase(float Seconds)
{
	if (!Take()) return false;
	return Check(nm_agent_tase(Agent, World, Seconds), TEXT("Tase"));
}

bool UDeliriumRagdollComponent::React(FString Line)
{
	return Take() && Check(nm_msg_send_text(Agent, World, TCHAR_TO_UTF8(*Line)), TEXT("React"));
}

bool UDeliriumRagdollComponent::HighFall()
{
	return Take() && Check(nm_agent_task_high_fall(Agent, World, nullptr, 0), TEXT("HighFall"));
}

bool UDeliriumRagdollComponent::CatchFall()
{
	return Take() && Check(nm_agent_task_catch_fall(Agent, nullptr), TEXT("CatchFall"));
}

bool UDeliriumRagdollComponent::FallDown(float Heading)
{
	if (!Take()) return false;
	const int32 R = nm_agent_task_fall_down(Agent, World, Heading);
	return Check(R, TEXT("FallDown")) && R == 1;
}

bool UDeliriumRagdollComponent::Grab(FVector Left, FVector LeftNormal, FVector Right, FVector RightNormal, float Hold)
{
	if (!Take()) return false;
	float L[6], R[6];
	ToNm(Left, L);
	ToNmDir(LeftNormal.GetSafeNormal(), L + 3);
	ToNm(Right, R);
	ToNmDir(RightNormal.GetSafeNormal(), R + 3);
	return Check(nm_agent_task_grab(Agent, World, LeftNormal.IsNearlyZero() ? nullptr : L, RightNormal.IsNearlyZero() ? nullptr : R, Hold), TEXT("Grab"));
}

bool UDeliriumRagdollComponent::JumpRoll()
{
	return Take() && Check(nm_agent_task_jump_roll(Agent, World), TEXT("JumpRoll"));
}

bool UDeliriumRagdollComponent::Flinch(FVector From)
{
	if (!Take()) return false;
	float F[3];
	ToNm(From, F);
	return Check(nm_agent_task_flinch(Agent, World, nullptr, F), TEXT("Flinch"));
}

bool UDeliriumRagdollComponent::Tackle(UDeliriumRagdollComponent* Target, bool bLow, bool bKeep)
{
	return Target && Target != this && Target->Take() && Take()
		&& Check(nm_agent_task_tackle(Agent, World, Target->Agent, bLow ? 1 : 0, bKeep ? 1 : 0), TEXT("Tackle"));
}

bool UDeliriumRagdollComponent::SetWater(float Level)
{
	return Agent && Check(nm_agent_set_water(Agent, World, Level / 100.f), TEXT("SetWater"));
}

FDeliriumState UDeliriumRagdollComponent::GetState() const
{
	FDeliriumState S;
	if (!Agent) return S;
	float P[3], V[3];
	nm_agent_get_com(Agent, P, V);
	float Head[3];
	int32 Up = 0;
	S.bRagdolled = true;
	S.bBalancing = nm_agent_is_balancing(Agent) == 1;
	S.bFallen = nm_agent_has_fallen(Agent) == 1;
	S.bCatchingFall = nm_agent_is_catching_fall(Agent) == 1;
	S.bRolledUp = nm_agent_is_rolled_up(Agent) == 1;
	S.bRelaxing = nm_agent_is_relaxing(Agent) == 1;
	S.bDead = nm_agent_is_dead(Agent) == 1;
	S.bInWater = nm_agent_in_water(Agent) == 1;
	S.bBurning = nm_agent_is_burning(Agent) == 1;
	S.bTased = nm_agent_is_tased(Agent) == 1;
	S.Tackling = nm_agent_tackle_state(Agent);
	S.LyingFor = nm_agent_get_lying(Agent, Head, &Up);
	S.Centre = FromNm(P);
	S.Velocity = FromNm(V);
	S.Mass = nm_agent_get_mass(Agent);
	return S;
}

void UDeliriumRagdollComponent::RegisterCar(AActor* Car, FVector Size, bool bPlayerDriven)
{
	RegisterMover(Car, EDeliriumMover::Car, Size, bPlayerDriven);
}

void UDeliriumRagdollComponent::RegisterMover(AActor* Car, EDeliriumMover Kind, FVector Size, bool bPlayerDriven, bool bBrace, bool bKnockDown, FVector Centre)
{
	if (!Car) return;
	UnregisterCar(Car);
	FVector Half = Size / 2.f;
	if (Size.IsNearlyZero())
	{
		const FBox B = Car->CalculateComponentsBoundingBoxInLocalSpace(true);
		if (!B.IsValid) { UE_LOG(LogDelirium, Warning, TEXT("RegisterCar %s: nothing to size it by: give a size"), *Car->GetName()); return; }
		Centre = B.GetCenter();
		Half = B.GetExtent();
	}
	CarList.Add({ Car, Centre, Half, Car->GetActorLocation(), FVector::ZeroVector, GFrameCounter, bPlayerDriven, Kind, bBrace, bKnockDown });
}

void UDeliriumRagdollComponent::UnregisterCar(AActor* Car)
{
	CarList.RemoveAll([Car](const FCar& C) { return C.Actor == Car; });
}

int32 UDeliriumRagdollComponent::Cars(AActor* Car)
{
	CarList.RemoveAll([](const FCar& C) { return !C.Actor.IsValid(); });
	if (CarList.Num() == 0) return Car ? -1 : 0;
	const float Dt = GetWorld() ? GetWorld()->GetDeltaSeconds() : 0.f;
	TArray<float> Boxes;
	Boxes.SetNumZeroed(CarList.Num() * 13);
	const FVector Here = GetOwner()->GetActorLocation();
	AActor* Coming = nullptr;
	for (int32 K = 0; K < CarList.Num(); K++)
	{
		FCar& C = CarList[K];
		const FTransform T = C.Actor->GetActorTransform();
		// Its speed once a frame (every ragdoll asks).
		if (C.Frame != GFrameCounter)
		{
			if (Dt > 0.f) C.Vel = (T.GetLocation() - C.Last) / Dt;
			C.Last = T.GetLocation();
			C.Frame = GFrameCounter;
		}
		const FVector At = T.TransformPosition(C.Centre);
		float* B = &Boxes[K * 13];
		ToNm(At, B);
		ToNmTurn(T.GetRotation(), B + 3);
		B[7] = C.Half.X / 100.f; B[8] = C.Half.Y / 100.f; B[9] = C.Half.Z / 100.f;
		ToNm(C.Vel, B + 10);
		// A quarter second ahead, a hand's width round it: a character there still on its feet goes to NM.
		if (!Agent && C.bKnockDown && C.Vel.Size() > 200.f)
		{
			const FVector L = T.InverseTransformPosition(Here - C.Vel * 0.25f) - C.Centre;
			if (FMath::Abs(L.X) < C.Half.X + 30.f && FMath::Abs(L.Y) < C.Half.Y + 30.f && FMath::Abs(L.Z) < C.Half.Z + 30.f) Coming = C.Actor.Get();
		}
	}
	if (Coming && Take())
	{
		const FCar* C = CarList.FindByPredicate([Coming](const FCar& X) { return X.Actor == Coming; });
		if (C && C->bBrace)
		{
			Bracing = Coming;
			BraceLeft = 3.f;
			Check(nm_agent_task_brace(Agent, World, nullptr), TEXT("Brace"));
		}
	}
	if (!Agent) return CarList.Num();
	Check(nm_world_set_boxes(World, Boxes.GetData(), CarList.Num()), TEXT("nm_world_set_boxes"));
	TArray<int32> Kinds;
	for (const FCar& C : CarList) Kinds.Add(int32(C.Kind) | (C.bPlayer ? NM_HIT_PLAYER : 0));
	Check(nm_world_set_box_kinds(World, Kinds.GetData(), Kinds.Num()), TEXT("nm_world_set_box_kinds"));
	if (AActor* Braced = Bracing.Get())
	{
		const int32 K = CarList.IndexOfByPredicate([Braced](const FCar& C) { return C.Actor == Braced; });
		if ((BraceLeft -= Dt) <= 0.f || K < 0) Bracing = nullptr;
		else
		{
			// Its frame in NM: the box's turned axes (y flips, so its y axis is Unreal's -Y), then its centre.
			const FTransform T = Braced->GetActorTransform();
			float M[12];
			ToNmDir(T.GetUnitAxis(EAxis::X), M);
			ToNmDir(-T.GetUnitAxis(EAxis::Y), M + 3);
			ToNmDir(T.GetUnitAxis(EAxis::Z), M + 6);
			ToNm(T.TransformPosition(CarList[K].Centre), M + 9);
			Check(nm_agent_task_brace_control(Agent, World, M, nullptr, nullptr), TEXT("Brace"));
		}
	}
	return Car ? CarList.IndexOfByPredicate([Car](const FCar& C) { return C.Actor == Car; }) : CarList.Num();
}

bool UDeliriumRagdollComponent::CarComing(AActor* Car)
{
	if (!Take()) return false;
	if (Cars(Car) < 0) { UE_LOG(LogDelirium, Warning, TEXT("CarComing: %s isn't registered (RegisterCar)"), Car ? *Car->GetName() : TEXT("null")); return false; }
	Bracing = Car;
	BraceLeft = 3.f;
	return Check(nm_agent_task_brace(Agent, World, nullptr), TEXT("CarComing"));
}

bool UDeliriumRagdollComponent::CarDoor(AActor* Car)
{
	if (!Take()) return false;
	const int32 K = Cars(Car);
	if (K < 0) { UE_LOG(LogDelirium, Warning, TEXT("CarDoor: %s isn't registered (RegisterCar)"), Car ? *Car->GetName() : TEXT("null")); return false; }
	return Check(nm_agent_task_car_door(Agent, World, K, nullptr), TEXT("CarDoor"));
}

bool UDeliriumRagdollComponent::Kill()
{
	return Take() && Check(nm_agent_set_dead(Agent, 1), TEXT("Kill")) && Check(nm_agent_task_relax(Agent, -1.f, -1.f), TEXT("Kill"));
}

bool UDeliriumRagdollComponent::Sever(FVector Point, FVector Impulse)
{
	if (!Take()) return false;
	float At[3], Imp[3];
	ToNm(Point, At);
	ToNmDir(Impulse, Imp);
	// Nearest the hips (the root) or cut already: nothing to cut.
	const int32 Part = nm_agent_nearest_part(Agent, At);
	if (Part <= 0 || nm_agent_is_severed(Agent, Part) == 1) return false;
	if (!Check(nm_agent_sever(Agent, World, Part, At, Imp), TEXT("Sever"))) return false;
	return true;
}

void UDeliriumRagdollComponent::BuildParts()
{
	const int32 N = nm_agent_part_count(Agent);
	for (int32 K = 0; K < N; K++)
	{
		float Radius = 0.f, Half = 0.f, Mass = 0.f, I[3] = {}, Box[3] = {}, P[3], Q[4];
		nm_build_part_info(Build, K, nullptr, &Radius, &Half, nullptr);
		UPrimitiveComponent* C;
		// NM's part X, Y, Z: the body's X, Z, Y.
		if (nm_agent_get_part_body(Agent, K, &Mass, I, Box) == 2)
		{
			UBoxComponent* B = NewObject<UBoxComponent>(GetOwner());
			B->SetBoxExtent(FVector(Box[0], Box[2], Box[1]) * 100.f * Scale);
			C = B;
		}
		else
		{
			UCapsuleComponent* B = NewObject<UCapsuleComponent>(GetOwner());
			B->SetCapsuleSize(Radius * 100.f * Scale, (Half + Radius) * 100.f * Scale);
			C = B;
		}
		nm_agent_get_part_transform(Agent, K, P, Q);
		C->SetWorldTransform(FTransform(FromNmTurn(Q) * PartToBody, FromNm(P)));
		// Kinematic (they don't meet each other or other ragdolls'), blocking physics objects; not the character's own
		// capsule and mesh.
		C->SetCollisionProfileName(UCollisionProfile::PhysicsActor_ProfileName);
		C->SetCollisionObjectType(ECC_PhysicsBody);
		C->SetCollisionResponseToChannel(ECC_Pawn, ECR_Ignore);
		C->SetSimulatePhysics(false);
		C->RegisterComponent();
		PartBodies.Add(C);
	}
	Look = 0.f;
}

void UDeliriumRagdollComponent::MoversNear()
{
	UWorld* W = GetWorld();
	if (!W || PartBodies.Num() == 0) return;
	TArray<FOverlapResult> Hits;
	W->OverlapMultiByObjectType(Hits, PartBodies[0]->GetComponentLocation(), FQuat::Identity, FCollisionObjectQueryParams::AllObjects,
		FCollisionShape::MakeSphere(500.f * Scale));
	for (const FOverlapResult& H : Hits)
	{
		UPrimitiveComponent* C = H.GetComponent();
		AActor* A = H.GetActor();
		// ponytail: only actors whose root simulates (a mover is tracked by its actor's place); others need RegisterMover
		// with their own actor.
		if (!C || !A || A == GetOwner() || !C->IsSimulatingPhysics() || A->GetRootComponent() != C) continue;
		if (CarList.ContainsByPredicate([A](const FCar& Car) { return Car.Actor == A; })) continue;
		RegisterMover(A, EDeliriumMover::Object, FVector::ZeroVector, false, false, true, FVector::ZeroVector);
	}
}

void UDeliriumRagdollComponent::GetUp()
{
	if (!Agent) return;
	float Head[3];
	int32 Up = 0;
	nm_agent_get_lying(Agent, Head, &Up);
	const FVector Hips = GetHips();
	const FVector Facing = FVector(Head[0], -Head[1], 0.f).GetSafeNormal() * (Up ? -1.f : 1.f);
	// The actor stands where the body lies, facing the way it'll get up.
	FVector At(Hips.X, Hips.Y, Ground(Hips));
	if (ACharacter* C = Cast<ACharacter>(GetOwner())) At.Z += C->GetSimpleCollisionHalfHeight();
	// The body stays where it lies while the actor moves under it: its bones' world frames, before and after.
	TArray<FTransform> Frames;
	for (int32 B : Bones) Frames.Add(Posed->GetBoneTransform(B));
	GetOwner()->SetActorLocationAndRotation(At, Facing.IsNearlyZero() ? Pinned.GetRotation() : Facing.Rotation().Quaternion(), false, nullptr, ETeleportType::TeleportPhysics);
	const int32 Parent = Posed->GetSkinnedAsset()->GetRefSkeleton().GetParentIndex(Bones[0]);
	FTransform& HipsLocal = Posed->BoneSpaceTransforms[Bones[0]];
	const FVector Size = HipsLocal.GetScale3D();
	HipsLocal = Frames[0].GetRelativeTransform(Parent >= 0 ? Posed->GetBoneTransform(Parent) : Posed->GetComponentTransform());
	HipsLocal.SetScale3D(Size);
	Posed->RefreshBoneTransforms();
	Last = Posed->BoneSpaceTransforms;
	UPoseableMeshComponent* Keep = Posed;
	Posed = nullptr;
	Release();
	Posed = Keep;
	if (GetUpBlend > 0.f)
	{
		// The animation runs on, unseen, while the copy blends into it.
		Mesh->SetVisibility(false, true);
		WasTick = Mesh->VisibilityBasedAnimTickOption;
		Mesh->VisibilityBasedAnimTickOption = EVisibilityBasedAnimTickOption::AlwaysTickPoseAndRefreshBones;
		Blend = GetUpBlend;
	}
	else
	{
		Posed->DestroyComponent();
		Posed = nullptr;
	}
}

void UDeliriumRagdollComponent::Release()
{
	if (Posed) { Posed->DestroyComponent(); Posed = nullptr; }
	for (UPrimitiveComponent* C : PartBodies) if (C) C->DestroyComponent();
	PartBodies.Reset();
	if (Mesh)
	{
		Mesh->SetVisibility(true, true);
		Mesh->bPauseAnims = false;
		if (bFollow) Mesh->VisibilityBasedAnimTickOption = FollowTick;
	}
	bFollow = false;
	if (ACharacter* C = Cast<ACharacter>(GetOwner()))
		if (C->GetCharacterMovement()->MovementMode == MOVE_None) C->GetCharacterMovement()->SetMovementMode(MOVE_Walking);
	if (Rig) nm_rig_destroy(Rig);
	Lead.Remove(Agent);
	if (Agent) nm_agent_destroy(Agent);
	if (Build) nm_build_destroy(Build);
	if (World)
	{
		const int32 I = Shared.IndexOfByPredicate([&](const FShared& W) { return W.World == World; });
		if (I >= 0 && Shared[I].Refs > 1) Shared[I].Refs--;
		else
		{
			if (I >= 0) Shared.RemoveAt(I);
			nm_world_destroy(World);
		}
	}
	Rig = nullptr;
	Agent = nullptr;
	Build = nullptr;
	World = nullptr;
}

void UDeliriumRagdollComponent::EndPlay(const EEndPlayReason::Type Reason)
{
	Release();
	Super::EndPlay(Reason);
}
