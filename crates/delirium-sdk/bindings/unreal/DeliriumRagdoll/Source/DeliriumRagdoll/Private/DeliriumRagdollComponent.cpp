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
#include "Engine/Engine.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/World.h"
#include "GameFramework/Character.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "Interfaces/IPluginManager.h"
#include "Misc/Paths.h"
#include "PhysicalMaterials/PhysicalMaterial.h"
#include "PhysicsEngine/BodySetup.h"
#include "Physics/PhysicsInterfaceCore.h"

DEFINE_LOG_CATEGORY_STATIC(LogDelirium, Log, All);

namespace
{
	NmBuild* BaseBuild = nullptr;
	/** Each kind of character's tuned build (null while it's being tuned). */
	TMap<FString, NmBuild*> Tuned;
	FCriticalSection TunedLock;

	/** Worlds ragdolls share, so they push and pile on each other: one per spot (the same Unreal world and size, ground
	 *  within 50 cm, within 5 m of where it was made, its level read there), freed with its last ragdoll. */
	struct FShared { NmWorld* World; float Scale; FVector At; int32 Refs; TWeakObjectPtr<UWorld> In; };
	TArray<FShared> Shared;

	/** RegisterCar's: the actor, its box (centre in its frame, half size, cm), its place and speed (once a frame); Comp
	 *  (EnginePhysics' movers) a simulating part of the actor tracked instead of the actor. */
	struct FCar { TWeakObjectPtr<AActor> Actor; FVector Centre, Half, Last, Vel; uint64 Frame; bool bPlayer; EDeliriumMover Kind; bool bBrace, bKnockDown; TWeakObjectPtr<UPrimitiveComponent> Comp;
		/** Its turn last frame (zero: none yet) and its spin (rad/s). */
		FQuat Turned = FQuat(0, 0, 0, 0); FVector Spin = FVector::ZeroVector; };
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

	/** A component's physical material as a surface row: 0 the engine's default, else one of rows 200 up, made for it the
	 *  first time (friction against the default's 0.7, restitution; nm_set_surface: ice's friction 0.05 slides). */
	uint8 Surface(const UPrimitiveComponent* C)
	{
		static TArray<TPair<float, float>> Rows;
		const UPhysicalMaterial* M = C ? const_cast<UPrimitiveComponent*>(C)->BodyInstance.GetSimplePhysicalMaterial() : nullptr;
		if (!M || M == GEngine->DefaultPhysMaterial || (M->Friction == 0.7f && M->Restitution == 0.3f)) return 0;
		const TPair<float, float> Key(M->Friction / 0.7f, M->Restitution);
		int32 K = Rows.IndexOfByKey(Key);
		if (K < 0)
		{
			K = Rows.Add(Key);
			// ponytail: 56 distinct materials; past that they share the last row.
			if (K < 56) nm_set_surface(200 + K, Key.Key, FMath::Clamp(Key.Value, 0.f, 1.f));
		}
		return uint8(200 + FMath::Min(K, 55));
	}

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
		TArray<uint8> Mats;
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
			const uint8 Mat = Surface(C);
			while (Mats.Num() < Tris.Num() / 3) Mats.Add(Mat);
		}
		if (Tris.Num() == 0) return 0;
		// The y flip mirrors them: turn each back to face the same way.
		for (int32 I = 0; I < Tris.Num(); I += 3) Swap(Tris[I + 1], Tris[I + 2]);
		Check(nm_world_add_mesh(World, Pts.GetData(), Pts.Num() / 3, Tris.GetData(), Tris.Num() / 3, Mats.GetData()), TEXT("nm_world_add_mesh"));
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
	// Native crashes land in the editor's or game's own dump; a panic in the SDK writes delirium_physics-crash.txt in Saved.
	nm_crash_report(TCHAR_TO_UTF8(*FPaths::ConvertRelativePathToFull(FPaths::ProjectSavedDir())));
	nm_set_step_limit(S->bStepLimit ? 1 : 0);
	nm_set_fire_reaction(S->bFireReaction ? 1 : 0);
	if (!S->Weapons.IsEmpty()) LoadWeapons(S->Weapons.EndsWith(TEXT(".xml")) ? Resolve(S->Weapons) : S->Weapons);
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
	// BoneMap's pairs first (None: no part), the name guess for the rest.
	TMap<FString, int32> Map;
	TArray<FString> Pairs;
	BoneMap.ParseIntoArray(Pairs, TEXT(","));
	for (const FString& P : Pairs)
	{
		FString Bone, Part;
		if (!P.Split(TEXT("="), &Bone, &Part)) continue;
		Part.TrimStartAndEndInline();
		int32 K = Part.Equals(TEXT("None"), ESearchCase::IgnoreCase) ? -1 : -2;
		for (int32 I = 0; I < 21 && K == -2; I++)
		{
			char Name[64] = {};
			nm_part_name(I, Name, 64);
			if (Part == UTF8_TO_TCHAR(Name)) K = I;
		}
		if (K != -2) Map.Add(Bone.TrimStartAndEnd(), K);
	}
	for (int32 B = 0; B < All; B++)
	{
		const FString Name = Ref.GetBoneName(B).ToString();
		const int32* Mapped = Map.Find(Name);
		Guess.Add(Mapped ? *Mapped : nm_rig_part_for_name(TCHAR_TO_UTF8(*Name)));
	}
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
		{ return W.In.Get() == GetWorld() && W.Scale == Scale && FMath::Abs(W.At.Z - Floor) < 50.f * Scale && FVector::Dist(W.At, Spot) < 500.f * Scale; });
	if (Near >= 0)
	{
		World = Shared[Near].World;
		Shared[Near].Refs++;
	}
	else
	{
		World = nm_world_create();
		Check(nm_world_set_scale(World, Scale), TEXT("nm_world_set_scale"));
		Shared.Add({ World, Scale, Spot, 1, GetWorld() });
		const int32 Tris = Level(GetWorld(), GetOwner(), Now, 1500.f, World);
		UE_LOG(LogDelirium, Log, TEXT("%s: %d level triangles"), *GetOwner()->GetName(), Tris);
		float F[3];
		ToNm(Spot, F);
		// The floor of the ground's material under him (ice stays ice past the level's reach).
		FHitResult Under;
		FCollisionQueryParams UnderQ(TEXT("DeliriumGround"), false, GetOwner());
		const uint8 FloorMat = GetWorld()->LineTraceSingleByObjectType(Under, Now, Now - FVector(0, 0, 500), FCollisionObjectQueryParams(ECC_WorldStatic), UnderQ) ? Surface(Under.GetComponent()) : 0;
		if (FloorMat == 0) Check(nm_world_add_floor(World, F[0], F[1], F[2], 100.f * Scale), TEXT("nm_world_add_floor"));
		else
		{
			const float Half = 100.f * Scale;
			const float Corners[12] = { F[0] - Half, F[1] - Half, F[2], F[0] + Half, F[1] - Half, F[2], F[0] + Half, F[1] + Half, F[2], F[0] - Half, F[1] + Half, F[2] };
			const uint32 Two[6] = { 0, 1, 2, 0, 2, 3 };
			const uint8 Mats[2] = { FloorMat, FloorMat };
			Check(nm_world_add_mesh(World, Corners, 4, Two, 2, Mats), TEXT("nm_world_add_floor"));
		}
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
	const float Strong = Strength > 0.f ? Strength : S->Strength;
	if (Strong != 1.f) Check(nm_agent_set_strength(Agent, Strong), TEXT("Strength"));
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

bool UDeliriumRagdollComponent::SetStrength(float K)
{
	Strength = K;
	return !Agent || Check(nm_agent_set_strength(Agent, K > 0.f ? K : GetDefault<UDeliriumSettings>()->Strength), TEXT("SetStrength"));
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

bool UDeliriumRagdollComponent::Task(TFunctionRef<int32()> F, const TCHAR* What)
{
	return Take() && Check(F(), What);
}

void UDeliriumRagdollComponent::Feed()
{
	// Follow's feed (Step hands NM the hidden mesh's pose each step while bFollow).
	if (bFollow || !Mesh) return;
	bFollow = true;
	FollowTick = Mesh->VisibilityBasedAnimTickOption;
	Mesh->VisibilityBasedAnimTickOption = EVisibilityBasedAnimTickOption::AlwaysTickPoseAndRefreshBones;
	Mesh->bPauseAnims = false;
	Incoming.SetNum(Bones.Num() * 7);
}

namespace
{
	/** A weapon of the table: its reaction set, and push, damage, pellets, ms between shots, clip, accuracy, range (m). */
	bool Weapon(const FString& Name, FString& Set, float* V)
	{
		char S[64] = {};
		const int32 K = nm_weapon_find(TCHAR_TO_UTF8(*Name));
		if (K < 0 || nm_weapon_get(K, nullptr, 0, S, sizeof S, V) < 0) return false;
		Set = UTF8_TO_TCHAR(S);
		return true;
	}

	/** A text call's answer (its buffer grown till it fits). */
	FString Text(TFunctionRef<int32(char*, int32)> F)
	{
		TArray<char> B;
		B.SetNumZeroed(1024);
		const int32 N = F(B.GetData(), B.Num());
		if (N >= B.Num()) { B.SetNumZeroed(N + 1); F(B.GetData(), B.Num()); }
		return N <= 0 ? FString() : FString(UTF8_TO_TCHAR(B.GetData()));
	}
}

float UDeliriumRagdollComponent::ShootWeapon(FString Name, FVector From, FVector Aim, float Health)
{
	FString Set;
	float V[7] = { 40.f, 0.f };
	if (!Weapon(Name, Set, V))
	{
		UE_LOG(LogDelirium, Warning, TEXT("ShootWeapon: no weapon \"%s\" in the Weapons setting; shot with the defaults, no damage"), *Name);
		Set = TEXT("*");
	}
	const float Left = FMath::Max(Health - V[1], 0.f);
	return ShootAlong(From, Aim, V[0], Set, Left) ? Left : -1.f;
}

int32 UDeliriumRagdollComponent::Shotgun(FVector From, FVector Aim, FString Name, float Health)
{
	static int32 Seed = 1;
	FString Set;
	float V[7];
	if (!Weapon(Name, Set, V)) { UE_LOG(LogDelirium, Warning, TEXT("Shotgun: no weapon \"%s\""), *Name); return -1; }
	if (!Take()) return -1;
	const int32 N = FMath::Max(int32(V[2]), 1);
	TArray<float> Dirs;
	Dirs.SetNumZeroed((N + 1) * 3);
	float A[3], F[3], At[3];
	ToNmDir(Aim.GetSafeNormal(), A);
	ToNm(From, F);
	const int32 Made = nm_shotgun_fan(A, V[5], V[6], N, &Seed, Dirs.GetData());
	TArray<float> Hits;
	for (int32 I = 1; I < Made; I++)
	{
		const int32 Part = nm_agent_aim_hit(Agent, F, &Dirs[I * 3], At);
		if (Part >= 0) Hits.Append({ float(Part), At[0], At[1], At[2], Dirs[I * 3], Dirs[I * 3 + 1], Dirs[I * 3 + 2] });
	}
	if (Hits.Num() == 0) return 0;
	const float Force = V[0] * GetDefault<UDeliriumSettings>()->ShotForce;
	return Check(nm_agent_task_shotgun(Agent, World, TCHAR_TO_UTF8(*Set), Hits.GetData(), Hits.Num() / 7, Health, Force), TEXT("Shotgun")) ? Hits.Num() / 7 : -1;
}

TArray<FString> UDeliriumRagdollComponent::WeaponNames()
{
	TArray<FString> Out;
	if (!Start()) return Out;
	for (int32 I = 0, N = nm_weapon_count(); I < N; I++) Out.Add(Text([I](char* B, int32 L) { return nm_weapon_get(I, B, L, nullptr, 0, nullptr) < 0 ? 0 : 1; }));
	return Out;
}

int32 UDeliriumRagdollComponent::LoadWeapons(FString T)
{
	const int32 R = T.EndsWith(TEXT(".xml")) ? nm_weapons_load_info(TCHAR_TO_UTF8(*Resolve(T))) : nm_weapons_parse(T.IsEmpty() ? nullptr : TCHAR_TO_UTF8(*T));
	Check(R, TEXT("Weapons"));
	return R;
}

bool UDeliriumRagdollComponent::Balance(FVector LookAt)
{
	float L[3];
	ToNm(LookAt, L);
	return Task([&] { return nm_agent_task_balance(Agent, World, nullptr, LookAt.IsZero() ? nullptr : L); }, TEXT("Balance"));
}

bool UDeliriumRagdollComponent::Stop() { return Task([&] { return nm_agent_task_stop(Agent); }, TEXT("Stop")); }

bool UDeliriumRagdollComponent::SetReactions(int32 Reactions) { return Task([&] { return nm_agent_set_reactions(Agent, Reactions); }, TEXT("SetReactions")); }

bool UDeliriumRagdollComponent::SetDive(float Gap) { return Task([&] { return nm_agent_set_dive(Agent, Gap); }, TEXT("SetDive")); }

bool UDeliriumRagdollComponent::Jump(bool bDive, FVector Direction)
{
	float D[3];
	ToNmDir(Direction.GetSafeNormal(), D);
	return Take() && nm_agent_jump(Agent, bDive, Direction.IsZero() ? nullptr : D) == 1;
}

bool UDeliriumRagdollComponent::NearMiss(FVector From, FVector Direction)
{
	float F[3], D[3];
	ToNm(From, F);
	ToNmDir(Direction.GetSafeNormal(), D);
	return Take() && nm_agent_near_miss(Agent, World, F, D) == 1;
}

bool UDeliriumRagdollComponent::ThrownAt(FVector At, FVector Velocity, bool bLook, bool bAvoid)
{
	float A[3], V[3];
	ToNm(At, A);
	ToNm(Velocity, V);
	return Task([&] { return nm_agent_thrown_at(Agent, World, A, V, bLook, bAvoid); }, TEXT("ThrownAt"));
}

bool UDeliriumRagdollComponent::Sit(FVector LookAt, FVector Seat)
{
	float L[3], S[3];
	ToNm(LookAt, L);
	ToNm(Seat, S);
	if (!Take()) return false;
	Feed();
	return Check(nm_agent_task_sit(Agent, World, LookAt.IsZero() ? nullptr : L, Seat.IsZero() ? nullptr : S), TEXT("Sit"));
}

bool UDeliriumRagdollComponent::TakePose(float Low, float High)
{
	if (!Take()) return false;
	Feed();
	return Check(nm_agent_task_pose(Agent, World, Low, High), TEXT("Pose"));
}

bool UDeliriumRagdollComponent::RollUpRelax(float Stiffness, float Damping, int32 RollMs)
{
	return Task([&] { return nm_agent_task_roll_up_relax(Agent, World, Stiffness, Damping, RollMs); }, TEXT("RollUpRelax"));
}

bool UDeliriumRagdollComponent::Script(int32 Ms) { return Task([&] { return nm_agent_task_script(Agent, Ms); }, TEXT("Script")); }

bool UDeliriumRagdollComponent::MuscleRise(bool bOn, float Help)
{
	if (!Take()) return false;
	Feed();
	return Check(nm_agent_muscle_rise(Agent, World, bOn, Help), TEXT("MuscleRise"));
}

bool UDeliriumRagdollComponent::LearnedGetUp(FString Net)
{
	const FString P = Resolve(Net.IsEmpty() ? GetDefault<UDeliriumSettings>()->GetUpNet : Net);
	return Task([&] { return nm_agent_learned_getup(Agent, TCHAR_TO_UTF8(*P)); }, TEXT("LearnedGetUp"));
}

bool UDeliriumRagdollComponent::LearnedWalk(bool bOn, FString Net, float Cycle)
{
	const FString P = Resolve(Net.IsEmpty() ? GetDefault<UDeliriumSettings>()->WalkNet : Net);
	return Task([&] { return nm_agent_learned_walk(Agent, World, bOn ? TCHAR_TO_UTF8(*P) : nullptr, Cycle); }, TEXT("LearnedWalk"));
}

bool UDeliriumRagdollComponent::SetStick(bool bOn, float X, float Y, float CameraYaw)
{
	// Unreal's yaw turns clockwise seen from above (Y flipped): NM's heading the other way.
	return Task([&] { return nm_agent_set_stick(Agent, bOn, X, Y, -FMath::DegreesToRadians(CameraYaw)); }, TEXT("SetStick"));
}

bool UDeliriumRagdollComponent::SetAttacker(AActor* Attacker, bool bArmed)
{
	float Them[12];
	if (Attacker)
	{
		UDeliriumRagdollComponent* D = Attacker->FindComponentByClass<UDeliriumRagdollComponent>();
		if (D && D->Agent)
		{
			const int32 Parts[4] = { 0, 7, 9, 12 };
			for (int32 K = 0; K < 4; K++) nm_agent_get_part_transform(D->Agent, Parts[K], Them + K * 3, nullptr);
		}
		else
		{
			// A person's pelvis, spine, chest and head heights, in its bounds.
			const FBox B = Attacker->GetComponentsBoundingBox();
			const float Tall = B.IsValid ? B.GetSize().Z : 180.f;
			const FVector Foot = B.IsValid ? FVector(B.GetCenter().X, B.GetCenter().Y, B.Min.Z) : Attacker->GetActorLocation();
			const float Up[4] = { 0.53f, 0.64f, 0.75f, 0.9f };
			for (int32 K = 0; K < 4; K++) ToNm(Foot + FVector(0, 0, Tall * Up[K]), Them + K * 3);
		}
	}
	return Task([&] { return nm_agent_set_attacker(Agent, Attacker ? Them : nullptr, bArmed); }, TEXT("SetAttacker"));
}

bool UDeliriumRagdollComponent::GunReady() { return Agent && nm_agent_gun_ready(Agent) == 1; }

int32 UDeliriumRagdollComponent::GetParam(FString Message, FString Param, float& Value, FVector& Vector, FString& Txt) const
{
	if (!Agent) return -1;
	float V[3] = {};
	const FTCHARToUTF8 M(*Message), P(*Param);
	const int32 K = nm_agent_get_param(Agent, M.Get(), P.Get(), V);
	if (K == 1) { Value = V[0]; return 0; }
	if (K == 3) { Vector = FVector(V[0], V[1], V[2]); return 3; }
	if (K != 0) return -1;
	char B[256] = {};
	if (nm_agent_get_param_string(Agent, M.Get(), P.Get(), B, sizeof B) < 0) return -1;
	Txt = UTF8_TO_TCHAR(B);
	return 4;
}

int32 UDeliriumRagdollComponent::Raycast(FVector From, FVector To, FVector& Point, FVector& Normal) const
{
	if (!Agent) return -1;
	float F[3], T[3], P[3], N[3];
	ToNm(From, F);
	ToNm(To, T);
	const int32 K = nm_agent_raycast(Agent, F, T, P, N);
	if (K >= 0) { Point = FromNm(P); Normal = FVector(N[0], -N[1], N[2]); }
	return K;
}

FString UDeliriumRagdollComponent::CheckSetup() const
{
	return Agent ? Text([this](char* B, int32 L) { return nm_agent_check(Agent, B, L); }) : FString();
}

TArray<FDeliriumMessage> UDeliriumRagdollComponent::Messages()
{
	TArray<FDeliriumMessage> Out;
	if (!Start()) return Out;
	for (int32 M = 0, N = nm_message_count(); M < N; M++)
	{
		FDeliriumMessage& Msg = Out.AddDefaulted_GetRef();
		Msg.Name = Text([M](char* B, int32 L) { return nm_message_name(M, B, L); });
		for (int32 P = 0, PN = nm_message_param_count(M); P < PN; P++)
		{
			FDeliriumParam& Q = Msg.Params.AddDefaulted_GetRef();
			Q.Name = Text([M, P](char* B, int32 L) { return nm_message_param_name(M, P, B, L); });
			Q.Kind = nm_message_param_kind(M, P);
			Q.bUsed = nm_message_param_used(M, P) == 1;
			Q.Doc = Text([M, P](char* B, int32 L) { return nm_message_param_doc(M, P, B, L); });
			float V[3] = {};
			const int32 K = nm_message_param_default(M, P, V);
			Q.Default = K == 3 ? FVector(V[0], V[1], V[2]) : K == 1 ? FVector(V[0], 0, 0) : FVector::ZeroVector;
		}
	}
	return Out;
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
	S.bSettled = nm_agent_is_settled(Agent) == 1;
	S.bAwake = nm_agent_is_awake(Agent) == 1;
	S.bJumping = nm_agent_jumping(Agent) == 1;
	S.bTaskRunning = nm_agent_task_running(Agent) == 1;
	S.bLearnedGetUp = nm_agent_learned_getup_running(Agent) == 1;
	S.bResting = nm_agent_is_resting(Agent, World) == 1;
	S.bHasStance = nm_agent_get_stance(Agent, P) == 1;
	if (S.bHasStance) S.Stance = FromNm(P);
	for (int32 K = 0; K < 21; K++) S.Contacts.Add(nm_agent_get_part_contact(Agent, K));
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
	CarList.RemoveAll([](const FCar& C) { return !C.Actor.IsValid() || (!C.Comp.IsExplicitlyNull() && !C.Comp.IsValid()); });
	if (CarList.Num() == 0) return Car ? -1 : 0;
	const float Dt = GetWorld() ? GetWorld()->GetDeltaSeconds() : 0.f;
	TArray<float> Boxes;
	Boxes.SetNumZeroed(CarList.Num() * 13);
	const FVector Here = GetOwner()->GetActorLocation();
	AActor* Coming = nullptr;
	for (int32 K = 0; K < CarList.Num(); K++)
	{
		FCar& C = CarList[K];
		const FTransform T = C.Comp.IsValid() ? FTransform(C.Comp->GetComponentQuat(), C.Comp->GetComponentLocation()) : C.Actor->GetActorTransform();
		// Its speed once a frame (every ragdoll asks).
		if (C.Frame != GFrameCounter)
		{
			const FQuat Turn = T.GetRotation();
			if (Dt > 0.f)
			{
				C.Vel = (T.GetLocation() - C.Last) / Dt;
				// Its spin: the turn since last frame, the short way round, over the frame.
				FQuat Delta = Turn * C.Turned.Inverse();
				if (Delta.W < 0.f) Delta = FQuat(-Delta.X, -Delta.Y, -Delta.Z, -Delta.W);
				FVector Axis;
				float Angle;
				Delta.ToAxisAndAngle(Axis, Angle);
				C.Spin = C.Turned.SizeSquared() > 0.f && Angle > 1e-6f ? Axis * (Angle / Dt) : FVector::ZeroVector;
			}
			C.Last = T.GetLocation();
			C.Turned = Turn;
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
	// A spin is a turn: (x, y, z) becomes (-x, y, -z), as a rotation's.
	TArray<float> Spins;
	for (const FCar& C : CarList) Spins.Append({ -float(C.Spin.X), float(C.Spin.Y), -float(C.Spin.Z) });
	Check(nm_world_set_box_spins(World, Spins.GetData(), CarList.Num()), TEXT("nm_world_set_box_spins"));
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

int32 UDeliriumRagdollComponent::Leash(FVector Point, FVector Anchor, float Length)
{
	if (!Take() || Length <= 0.f) return -1;
	float At[3], To[3];
	ToNm(Point, At);
	ToNm(Anchor, To);
	const int32 Part = nm_agent_nearest_part(Agent, At);
	if (Part < 0 || !Check(nm_agent_leash(Agent, Part, At, To, Length / 100.f), TEXT("Leash"))) return -1;
	return Part;
}

bool UDeliriumRagdollComponent::LeashLength(int32 Part, float Length)
{
	return Take() && Check(nm_agent_leash(Agent, Part, nullptr, nullptr, Length / 100.f), TEXT("LeashLength"));
}

bool UDeliriumRagdollComponent::LeashAnchor(int32 Part, FVector Anchor, float Length)
{
	float To[3];
	ToNm(Anchor, To);
	return Take() && Check(nm_agent_leash(Agent, Part, nullptr, To, Length / 100.f), TEXT("LeashAnchor"));
}

bool UDeliriumRagdollComponent::Unleash(int32 Part)
{
	return Agent && Check(nm_agent_unleash(Agent, Part), TEXT("Unleash"));
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
		if (!C || !A || A == GetOwner() || !C->IsSimulatingPhysics()) continue;
		if (CarList.ContainsByPredicate([A, C](const FCar& Car) { return Car.Actor == A && (Car.Comp.IsExplicitlyNull() || Car.Comp == C); })) continue;
		if (A->GetRootComponent() == C) { RegisterMover(A, EDeliriumMover::Object, FVector::ZeroVector, false, false, true, FVector::ZeroVector); continue; }
		// A simulating part that isn't the root moves apart from its actor: tracked by itself, boxed by its own bounds
		// (on its axes, at its scale).
		const FBoxSphereBounds L = C->CalcBounds(FTransform(FQuat::Identity, FVector::ZeroVector, C->GetComponentScale()));
		CarList.Add({ A, L.Origin, L.BoxExtent, C->GetComponentLocation(), FVector::ZeroVector, GFrameCounter, false, EDeliriumMover::Object, false, true, C });
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
		if (UCharacterMovementComponent* M = C->GetCharacterMovement(); M && M->MovementMode == MOVE_None) M->SetMovementMode(MOVE_Walking);
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

void UDeliriumRagdollComponent::OnComponentDestroyed(bool bDestroyingHierarchy)
{
	// A world torn down without play (no EndPlay) still frees its ragdolls.
	if (Agent || World) Release();
	Super::OnComponentDestroyed(bDestroyingHierarchy);
}
