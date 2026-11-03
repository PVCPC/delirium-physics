#pragma once

#include "Components/ActorComponent.h"
#include "DeliriumRagdollComponent.generated.h"

struct NmBuild;
struct NmWorld;
struct NmAgent;
struct NmRig;
class USkeletalMeshComponent;
class UPoseableMeshComponent;

DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FDeliriumLyingStill, bool, bFaceUp, FVector, Facing);

/**
 * Add to a character: its skeletal mesh becomes an active ragdoll when hit (keeps its balance, staggers, catches its
 * fall, lies still, gets up). While ragdolled the actor's movement and animation are paused and a posed copy of the
 * mesh shows the body; GetUp hands it back. Health stays yours: pass it in with each hit.
 */
UCLASS(ClassGroup = Physics, meta = (BlueprintSpawnableComponent))
class DELIRIUMRAGDOLL_API UDeliriumRagdollComponent : public UActorComponent
{
	GENERATED_BODY()

public:
	UDeliriumRagdollComponent();

	/** The mesh to ragdoll; empty: the actor's first skeletal mesh. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	TObjectPtr<USkeletalMeshComponent> Mesh;

	/** Raised once it has lain still half a second; `Facing` is the way to face once up (flat). */
	UPROPERTY(BlueprintAssignable, Category = "Delirium")
	FDeliriumLyingStill OnLyingStill;

	/** Seconds to blend from the ragdoll's pose into the animation when it gets up. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float GetUpBlend = 0.4f;

	/** Seconds lying still before it gets up by itself (0: never; call GetUp). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float GetUpAfter = 0.f;

	/** The body (the Playroom's): Fitted (the skeleton's own); Small, Average, Large, Lanky, Fat, Stocky, Waddler, Guard,
	 *  Brute or Brawler (that shape, of this character); Random; Custom (CustomBody). The mesh keeps its look: pick
	 *  one near it. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body", meta = (GetOptions = "Bodies"))
	FString Body = TEXT("Fitted");

	/** Random's seed: the same seed, the same body (0: each character its own). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	int32 BodySeed = 0;

	/** Random's spread, 1 to 4 (1.5 the Playroom's default). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body", meta = (ClampMin = "1", ClampMax = "4"))
	float BodyVariance = 1.5f;

	/** Random: the right side the left's. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	bool bBodySymmetric = true;

	/** Custom's sizes against the fitted body: legs, arms, trunk, width, head, neck, weight (kg), height (m; 0 keeps it). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	FString CustomBody = TEXT("1, 1, 1, 1, 1, 1, 77, 0");

	/** Behaviour parameters changed for this character (the Playroom's Parameters tab), every message of that behaviour
	 *  sent with them: "shot.bodyStiffness=7, bodyBalance.useHeadLook=0". SetOverride does one in code. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	FString Overrides;

	/** Every `Behaviour` message to it from now on carries `Param` as `Value` (on/off: 0 or 1). False (the reason in the
	 *  log) for a parameter it doesn't take. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetOverride(FString Behaviour, FString Param, float Value);

	/** Drops all overrides (Overrides too): its messages go out as sent again. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	void ClearOverrides();

	UFUNCTION()
	static TArray<FString> Bodies();

	/** Takes the character (no hit): it stands and keeps its balance. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Ragdoll();

	/** A bullet: `Health` left after it (0-200: 100+ staggers, under 100 goes down, under 50 is a kill). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Shoot(FVector Point, FVector Direction, float Force = 40.f, FString Set = TEXT("POWERFUL_WEAPON"), float Health = 200.f);

	/** A bullet along `Aim` from `From`: strikes the body part the aim passes. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool ShootAlong(FVector From, FVector Aim, float Force = 40.f, FString Set = TEXT("POWERFUL_WEAPON"), float Health = 200.f);

	/** A push at the chest, N s (60 is a solid shove; impulses are N s, places cm). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Push(FVector Impulse);

	/** An explosion at `At` (cm), thrown at up to `Speed` m/s. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Explode(FVector At, float Radius = 500.f, float Speed = 15.f);

	/** A blow that doesn't kill, looking at the attacker. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool MeleeHit(FVector Point, FVector Impulse, FVector AttackerHead, bool bTimid = false);

	/** Catches fire; call with false once out. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool OnFire(bool bBurning = true, float Health = 200.f);

	/** Falling from a height: arms out, legs ready. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool HighFall();

	/** Goes limp and stays down. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Kill();

	/** Cuts off the limb nearest `Point` (hide its mesh and spawn your gib yourself). False nearest the hips or cut already. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Sever(FVector Point, FVector Impulse);

	/** Hands it back: the actor stands where the body lies, its mesh, animation and movement back on. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	void GetUp();

	UFUNCTION(BlueprintPure, Category = "Delirium")
	bool IsRagdolled() const { return Agent != nullptr; }

	/** The hips bone's place while ragdolled. */
	UFUNCTION(BlueprintPure, Category = "Delirium")
	FVector GetHips() const;

	/** Steps it (TickComponent does, at 60 a second); for tests. */
	void Step(float Seconds);

	virtual void TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction) override;
	virtual void EndPlay(const EEndPlayReason::Type Reason) override;

	/** Loads the SDK (once; Project Settings > Plugins > Delirium Ragdoll); false with the reason in the log. */
	static bool Start();

private:
	bool Take();
	void Pose();
	void Release();
	float Ground(const FVector& Hips) const;

	/** SetOverride's, on top of Overrides. */
	TArray<TTuple<FString, FString, float>> Added;
	NmBuild* Build = nullptr;
	NmWorld* World = nullptr;
	NmAgent* Agent = nullptr;
	NmRig* Rig = nullptr;
	UPROPERTY()
	TObjectPtr<UPoseableMeshComponent> Posed;
	/** Our bones (the hips and everything under them), as indices into the mesh's skeleton. */
	TArray<int32> Bones;
	TArray<float> PoseOut;
	float Scale = 1.f;
	float Clock = 0.f;
	/** Seconds of the get-up blend left, and the ragdoll's last pose (bone space) it blends from. */
	float Blend = 0.f;
	TArray<FTransform> Last;
	EVisibilityBasedAnimTickOption WasTick = EVisibilityBasedAnimTickOption::AlwaysTickPose;
	bool bTold = false;
	FTransform Pinned;
};
