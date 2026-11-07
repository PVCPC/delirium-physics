#pragma once

#include "Components/ActorComponent.h"
#include "Components/SkinnedMeshComponent.h"
#include "Components/PrimitiveComponent.h"
#include "DeliriumRagdollComponent.generated.h"

struct NmBuild;
struct NmWorld;
struct NmAgent;
struct NmRig;
class USkeletalMeshComponent;
class UPoseableMeshComponent;

DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FDeliriumLyingStill, bool, bFaceUp, FVector, Facing);
DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FDeliriumCarHit, float, Damage, AActor*, Car);
DECLARE_DYNAMIC_MULTICAST_DELEGATE_ThreeParams(FDeliriumFeedback, FString, Name, int32, Slot, float, Value);
DECLARE_DYNAMIC_MULTICAST_DELEGATE(FDeliriumFell);

/** What a ragdoll is doing now (GetState). */
USTRUCT(BlueprintType)
struct FDeliriumState
{
	GENERATED_BODY()

	/** Under NM at all (the rest is false/zero when not). */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bRagdolled = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bBalancing = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bFallen = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bCatchingFall = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bRolledUp = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bRelaxing = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bDead = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bInWater = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bBurning = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bTased = false;
	/** 2 diving at its tackle target, 1 holding on, 0 neither. */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") int32 Tackling = 0;
	/** Seconds lain still (0 while moving). */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") float LyingFor = 0.f;
	/** Centre of mass (cm) and its velocity (cm/s); the whole mass (kg). */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FVector Centre = FVector::ZeroVector;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FVector Velocity = FVector::ZeroVector;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") float Mass = 0.f;
	/** Settled (still and balanced), awake (its physics running), jumping, a task running, the learned get-up running,
	 *  resting (lying still on the ground). */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bSettled = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bAwake = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bJumping = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bTaskRunning = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bLearnedGetUp = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bResting = false;
	/** Where the balancer stands (between its feet, cm); bHasStance false when it isn't balancing. */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bHasStance = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FVector Stance = FVector::ZeroVector;
	/** Each of the 21 parts' contact flags (0: touching nothing). */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") TArray<int32> Contacts;
};

/** A parameter of a message NM takes (Messages). */
USTRUCT(BlueprintType)
struct FDeliriumParam
{
	GENERATED_BODY()
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FString Name;
	/** 0 float, 1 int, 2 bool, 3 vector, 4 string. */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") int32 Kind = 0;
	/** The game's behaviours read it. */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") bool bUsed = false;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FString Doc;
	/** Its default: a number's in X, a vector's (NM's space: metres, Y flipped) whole. */
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FVector Default = FVector::ZeroVector;
};

/** A message NM takes (React) and its parameters. */
USTRUCT(BlueprintType)
struct FDeliriumMessage
{
	GENERATED_BODY()
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") FString Name;
	UPROPERTY(BlueprintReadOnly, Category = "Delirium") TArray<FDeliriumParam> Params;
};

/** The Playroom's extra reactions, on top of the game's (SetReactions; each its own bit). */
UENUM(BlueprintType, meta = (Bitflags, UseEnumValuesAsMaskValuesInEditor = "true"))
enum class EDeliriumReaction : uint8
{
	None = 0 UMETA(Hidden),
	Windmill = 1 << 0,
	CurlUp = 1 << 1,
	CurlShot = 1 << 2,
	LookHit = 1 << 3,
	Ledge = 1 << 4,
	SmartFall = 1 << 5,
	Dodge = 1 << 6,
	Wall = 1 << 7,
};

/** What a registered mover is, for the game's damage and reaction when it hits. */
UENUM(BlueprintType)
enum class EDeliriumMover : uint8
{
	/** A car, a truck, a platform. */
	Car = 0,
	/** A crate, a door, debris: half a car's damage, a hard hit unless the character stands on it. */
	Object = 1,
	/** The train's own hurt, ahead of its front. */
	Train = 3,
};

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

	/** A registered car hit it: the health it takes (take it off yourself) and the car. */
	UPROPERTY(BlueprintAssignable, Category = "Delirium")
	FDeliriumCarHit OnCarHit;

	/** NM's own messages from its behaviours as they happen (the balancer giving up, a catch fall landing...): the
	 *  Playroom's Feedback list shows what each sends. */
	UPROPERTY(BlueprintAssignable, Category = "Delirium")
	FDeliriumFeedback OnFeedback;

	/** It lost its feet (the balancer gave up). Once per fall. */
	UPROPERTY(BlueprintAssignable, Category = "Delirium")
	FDeliriumFell OnFell;

	/** What it's doing now. */
	UFUNCTION(BlueprintPure, Category = "Delirium")
	FDeliriumState GetState() const;

	/** Its health now, for the game's reaction to a mover's hit (keep it with your character's). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float HitHealth = 200.f;

	/** The game's agility (0-255): under 3 is weak, it goes down sooner when hit. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	int32 HitAgility = 5;

	/** Seconds to blend from the ragdoll's pose into the animation when it gets up. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float GetUpBlend = 0.4f;

	/** Seconds lying still before it gets up by itself (0: never; call GetUp). */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float GetUpAfter = 0.f;

	/** The body (the Playroom's): Fitted (the skeleton's own); Small, Average, Large, Lanky, Fat, Stocky, Waddler, Guard,
	 *  Brute, Brawler or Bruiser (that shape, of this character); Random; Custom (CustomBody). The mesh keeps its look: pick
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

	/** This character's muscles times this (0: the project settings' Strength). 1.3-2 for a heavy custom body that can't
	 *  take a hit; under 1 for a weak one. SetStrength changes it while ragdolled. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	float Strength = 0.f;

	/** Bones the name guess misses or gets wrong, as BoneName=Part pairs: "Pelvis_jnt=Buttocks, Torso=Spine2, Tail=None".
	 *  Parts: Buttocks, Thigh_Left, Shin_Left, Foot_Left, Thigh_Right, Shin_Right, Foot_Right, Spine0-3, Neck, Head, Clavicle_Left, Upper_Arm_Left, Lower_Arm_Left, Hand_Left and the same _Right. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium|Body")
	FString BoneMap;

	/** Its muscles times K from now on (Strength; 0: the settings'). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetStrength(float K);

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

	/** A taser's hit: a current through it for Seconds (it jolts and locks, the Playroom's), ending by itself; 0 ends it now. No damage. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Tase(float Seconds = 2.5f);

	/** Any reaction, written as the Playroom console's line: its NM message name, then name=value params ("rollUp start=true", "bodyWrithe start=true armStiffness=12", "stopAllBehaviours"): ragdolls it first if
	 *  it isn't one. Vectors are x,y,z in NM's space (Z up, metres). False when NM refuses the line. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool React(FString Line);

	/** Falling from a height: arms out, legs ready. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool HighFall();

	/** Catching itself as it falls: hands out, then down (the game's catchFall). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool CatchFall();

	/** Pushed off a ledge (a drop, a low wall, stairs): looks 2 m out along `Heading` (0-1 of a turn round from behind;
	 *  negative: a random one) and goes over when there's one. False when there's none. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool FallDown(float Heading = -1.f);

	/** Grabs hold (a ledge, a car's side): each hand reaches for its point (cm; the surface's normal out of it, zero
	 *  leaves that hand free), has 1 s to get hold, then holds `Hold` s (0: until another reaction). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Grab(FVector Left, FVector LeftNormal, FVector Right, FVector RightNormal, float Hold = 0.f);

	/** Jumping out of a moving car: tucks and rolls. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool JumpRoll();

	/** Flinching from a threat at `From` (cm). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Flinch(FVector From);

	/** Dives at `Target`'s character (both ragdolled first, within 4 m): hands latch onto its chest (or thighs, `bLow`),
	 *  drives on and both go down. `bKeep`: never lets go by itself. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Tackle(UDeliriumRagdollComponent* Target, bool bLow = false, bool bKeep = false);

	/** Water under the ragdoll: its surface's height (cm; NaN: none). Down in it, it floats face up. Set it as it moves
	 *  over water. False when it isn't a ragdoll. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetWater(float Level);

	/** A shot from the Weapons table along the shooter's aim (as ShootAlong): its reaction, push and damage. Returns the
	 *  health left (`Health` less its damage), or -1 when there's no ragdoll. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	float ShootWeapon(FString Weapon, FVector From, FVector Aim, float Health = 200.f);

	/** A shotgun blast from the table's `Weapon` along the aim: its pellets fanned as the game's (its accuracy and range),
	 *  those striking him pushing as one shot. Returns how many struck (-1: refused). `Health` his after the blast. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	int32 Shotgun(FVector From, FVector Aim, FString Weapon = TEXT("shotgun"), float Health = 150.f);

	/** The weapons table's names (the default pistol, smg, rifle, shotgun, or the Weapons setting's). */
	UFUNCTION(BlueprintPure, Category = "Delirium")
	static TArray<FString> WeaponNames();

	/** The weapons table from text (as the Weapons setting; empty: the default four) or a weaponinfo.xml. Returns how many
	 *  (negative: unreadable, the reason in the log). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	static int32 LoadWeapons(FString Text);

	/** Its balance (the game's TaskNMBalance), looking at `LookAt` (zero: straight on). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Balance(FVector LookAt = FVector::ZeroVector);

	/** Stops the task it's running (Sit, Pose, Script, ...). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Stop();

	/** The Playroom's extra reactions on (EDeliriumReaction bits; 0: the game's alone). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetReactions(UPARAM(meta = (Bitmask, BitmaskEnum = "/Script/DeliriumRagdoll.EDeliriumReaction")) int32 Reactions);

	/** Shot at again within `Gap` s of a near miss, it dives (NearMiss). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetDive(float Gap = 3.5f);

	/** The Playroom's jump on its balance (loads, pushes off, flies, lands); `bDive` head first along `Direction` (zero:
	 *  its facing). False when it can't (not balancing). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Jump(bool bDive = false, FVector Direction = FVector::ZeroVector);

	/** A bullet that missed it, from `From` along `Direction`: close by its head or chest it flinches and looks there.
	 *  True when it noticed. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool NearMiss(FVector From, FVector Direction);

	/** Something thrown or dropped at it from `At` moving at `Velocity` (cm/s): `bLook` it follows it with its head,
	 *  `bAvoid` it flinches from it. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool ThrownAt(FVector At, FVector Velocity, bool bLook = true, bool bAvoid = true);

	/** Sits (the game's TaskNMSit; the animation's pose as its clip), looking at `LookAt`, at `Seat` (zero: straight on,
	 *  where it is). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Sit(FVector LookAt = FVector::ZeroVector, FVector Seat = FVector::ZeroVector);

	/** Its muscles take the animation's pose (the game's TaskNMPose) at a body stiffness from `Low` to `High` (negative: 4),
	 *  until Stop. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool TakePose(float Low = -1.f, float High = -1.f);

	/** Rolls up for `RollMs`, then goes limp at `Stiffness` / `Damping` (negative: the game's). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool RollUpRelax(float Stiffness = -1.f, float Damping = -1.f, int32 RollMs = 1000);

	/** The ragdoll is yours for `Ms` (0: until Stop): moved only by the messages you send (React). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Script(int32 Ms = 0);

	/** Gets up with its own muscles, the animation's get-up clip as its pose and a spring helping its pelvis at most `Help`
	 *  of its weight (negative: half); it can fail. `bOn` false: done, it balances. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool MuscleRise(bool bOn = true, float Help = -1.f);

	/** Lying, it gets up as the learned network did (`Net`, empty: the GetUpNet setting's): 8 s with its own muscles. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool LearnedGetUp(FString Net = TEXT(""));

	/** Walks under the stick (SetStick) with the learned network tuning its balance (`Net`, empty: the WalkNet setting's;
	 *  `Cycle` the walk clip's length it learned with). `bOn` false: its balance as it was. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool LearnedWalk(bool bOn = true, FString Net = TEXT(""), float Cycle = 1.f);

	/** The game's drunk control: pushes it along the stick (`X`, `Y` -1..1, up the stick forward) from the camera's yaw
	 *  `CameraYaw` (degrees); `bOn` false lets go. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetStick(bool bOn, float X = 0.f, float Y = 0.f, float CameraYaw = 0.f);

	/** Who it's up against: it looks at `Attacker`'s head (a ragdoll's, else a person's height in its bounds) and, `bArmed`,
	 *  points its gun at it (GunReady says when to fire). Null: no one. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool SetAttacker(AActor* Attacker, bool bArmed = false);

	/** Armed against an attacker: true once it's ready to fire (you fire the shot). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool GunReady();

	/** A behaviour's parameter as it runs now (as last sent, else its default): its kind (0 float, 3 vector, 4 string;
	 *  -1 none), the value in `Value`, `Vector` (NM's space) or `Text`. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	int32 GetParam(FString Message, FString Param, float& Value, FVector& Vector, FString& Text) const;

	/** The body part a segment hits first (and where, its normal); -1 when none. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	int32 Raycast(FVector From, FVector To, FVector& Point, FVector& Normal) const;

	/** What's off in its setup (one line each; empty: nothing). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	FString CheckSetup() const;

	/** The messages NM takes (React) and their parameters. */
	UFUNCTION(BlueprintPure, Category = "Delirium")
	static TArray<FDeliriumMessage> Messages();

	/** Anything you call a car (a car, a truck, a platform, a wrecking ball): ragdolls can't go into its box and meet it
	 *  at its speed, a standing character it's about to hit is ragdolled, and a hit reacts as the game's (brace, roll up,
	 *  stagger, limp) with OnCarHit. `Size`: its box (cm; zero: its components' bounds); `bPlayerDriven`: the player
	 *  drives it (the game's reactions differ). Unregister it when it's gone. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	static void RegisterCar(AActor* Car, FVector Size = FVector::ZeroVector, bool bPlayerDriven = false);

	UFUNCTION(BlueprintCallable, Category = "Delirium")
	static void UnregisterCar(AActor* Car);

	/** Anything that moves and hits characters, as `Kind`: as RegisterCar, and `bBrace`: a standing character in its way
	 *  braces against it as it's ragdolled (usually off for objects); `bKnockDown`: a standing character in its way is
	 *  ragdolled at all (off: only ragdolls meet it). `Centre`: the box's centre in its frame (cm), with `Size`. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	static void RegisterMover(AActor* Car, EDeliriumMover Kind, FVector Size = FVector::ZeroVector, bool bPlayerDriven = false,
		bool bBrace = true, bool bKnockDown = true, FVector Centre = FVector::ZeroVector);

	/** A registered car is about to hit it (your AI saw it coming): it braces against it, hands out (ArmsOutEarly: well
	 *  before), maybe a jump, until the hit or 3 s. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool CarComing(AActor* Car);

	/** Dragged from a registered car: both hands on its side, kicking and writhing once it moves, letting go at 12 m/s
	 *  or after 6 s. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool CarDoor(AActor* Car);

	/** Goes limp and stays down. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Kill();

	/** A leash: the part nearest `Point` tied to `Anchor`, at most `Length` cm away (slack inside, no stretch). Returns the part (for LeashLength / LeashAnchor / Unleash), -1 if none. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	int32 Leash(FVector Point, FVector Anchor, float Length);

	/** Leash `Part`'s new length in cm (shorter reels him in at 2 m/s). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool LeashLength(int32 Part, float Length);

	/** Leash `Part`'s anchor moved (a moving holder, or the corner it wraps round: take what's used up off the length). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool LeashAnchor(int32 Part, FVector Anchor, float Length);

	/** Lets leash `Part` go (-1: every one). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Unleash(int32 Part = -1);

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
	virtual void OnComponentDestroyed(bool bDestroyingHierarchy) override;

	/** Loads the SDK (once; Project Settings > Plugins > Delirium Ragdoll); false with the reason in the log. */
	static bool Start();

private:
	bool Take();
	void Pose();
	void Release();
	float Ground(const FVector& Hips) const;
	/** EnginePhysics: NM's parts as kinematic Chaos bodies where NM has them: NM's own physics moves him (Chaos's joints
	 *  and drives couldn't hold its short or heavy bodies up: they lack the original engine's joint armature), the bodies follow (Step)
	 *  and push Chaos's bodies. */
	void BuildParts();
	/** Sit, Pose, MuscleRise: the hidden mesh animates on for NM to take its pose (as Follow's). */
	void Feed();
	/** One of the game's tasks on it (ragdolled first); false when it couldn't be. */
	bool Task(TFunctionRef<int32()> F, const TCHAR* What);
	/** EnginePhysics: physics objects (simulating roots) within 5 m not yet registered, to NM as Object movers. */
	void MoversNear();

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
	/** Follow: the hidden mesh's animated bones each tick (nm_rig_incoming), and its tick option before. */
	TArray<float> Incoming;
	bool bFollow = false;
	/** EnginePhysics: NM's parts as kinematic bodies (each turned so its capsule runs along Z); empty in NM's own physics.
	 *  The seconds to the next look for movers near him. */
	UPROPERTY(Transient)
	TArray<TObjectPtr<UPrimitiveComponent>> PartBodies;
	float Look = 0.f;
	EVisibilityBasedAnimTickOption FollowTick = EVisibilityBasedAnimTickOption::AlwaysTickPose;
	float Scale = 1.f;
	float Clock = 0.f;
	/** Seconds of the get-up blend left, and the ragdoll's last pose (bone space) it blends from. */
	float Blend = 0.f;
	TArray<FTransform> Last;
	EVisibilityBasedAnimTickOption WasTick = EVisibilityBasedAnimTickOption::AlwaysTickPose;
	bool bTold = false;
	/** OnFell was raised for this fall. */
	bool bDown = false;
	FTransform Pinned;
	/** CarComing's car, and seconds left bracing against it. */
	TWeakObjectPtr<AActor> Bracing;
	float BraceLeft = 0.f;
	/** The registered cars into this ragdoll's world; the index of `Car` among them (-1: none). */
	int32 Cars(AActor* Car = nullptr);
};
