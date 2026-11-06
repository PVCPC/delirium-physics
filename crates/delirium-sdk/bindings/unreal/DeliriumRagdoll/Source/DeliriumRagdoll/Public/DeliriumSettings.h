#pragma once

#include "Engine/DeveloperSettings.h"
#include "DeliriumSettings.generated.h"

/** Project Settings > Plugins > Delirium Ragdoll. Paths are absolute, or under the project folder. */
UCLASS(Config = Game, DefaultConfig, meta = (DisplayName = "Delirium Ragdoll"))
class DELIRIUMRAGDOLL_API UDeliriumSettings : public UDeveloperSettings
{
	GENERATED_BODY()

public:
	/** The player's own GTAIV.exe or default.xex (the numbers are read from it). Empty: already unlocked. */
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString GameExe;

	/** The character file. */
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString Character = TEXT("character.xrt");

	/** The build dump. */
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString Build = TEXT("build.txt");

	/** The tuning file (optional). */
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString TaskParams;

	/** Real: the body is the character's own size. Homer: a person-sized body in a scaled world (giants, small
	 *  creatures). Auto: Real within 0.9-1.1 of a person's legs, Homer otherwise. */
	UPROPERTY(Config, EditAnywhere, Category = "Body", meta = (GetOptions = "SizeModes"))
	FString Size = TEXT("Auto");

	/** Every body's muscles times this (1: as made; 1.5-2 for tough enemies or a movie giant). */
	UPROPERTY(Config, EditAnywhere, Category = "Body", meta = (ClampMin = "0.1", ClampMax = "10"))
	float Strength = 1.f;

	/** Search each kind of character's balance settings once, in the background (kept in Saved/delirium-tunes.txt). */
	UPROPERTY(Config, EditAnywhere, Category = "Body")
	bool bAdaptive = true;

	/** Re-centre each joint's range on the pose it is handed over in (for rigs whose animation bends joints past NM's ranges: they would snap on the first step). Not the game's. */
	UPROPERTY(Config, EditAnywhere, Category = "Body")
	bool bRecentreJoints = false;

	/** Joints held at least this stiff, damped and gravity-opposing (0-1), whatever the reactions set: mask=stiffness:damping:gravity, comma separated (masks as setStiffness's: fb whole body, ua upper arms, ub upper body, lb lower body). fb=6:1:0.5 keeps a limp body from folding; empty: off. */
	UPROPERTY(Config, EditAnywhere, Category = "Body")
	FString Hold;

	/** Joints that keep playing the animation while the rest ragdolls (a boxer's guard, a held gun): mask=weight, comma separated (1 all the way, 0.5 halfway; masks as Hold's, ua upper arms). The animation keeps running under the ragdoll. Give those joints strength with Hold (ua=10:1:1). Empty: off. */
	UPROPERTY(Config, EditAnywhere, Category = "Body")
	FString Follow;

	/** A shot nearest the head kills. */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bHeadshotKills = true;

	/** Ragdolls try to keep their feet (off: they go limp). */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bBalance = true;

	/** Bracing for a car (or anything coming at it), both arms thrown out straight well before it hits, as the original demo (the Playroom's 'Arms out early when bracing'). Off: the game's, a late bend. */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bArmsOutEarly = false;

	/** His parts as Chaos bodies: NM's own physics still moves him (the game's exactly), his parts follow it as kinematic bodies pushing your physics objects, and physics objects within 5 m hit him (as Object movers). Off: NM's own physics alone, the level read in. */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bEnginePhysics = false;

	/** Every bullet's, blow's and explosion's push times these. */
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float ShotForce = 1.f;
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float MeleeForce = 1.f;
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float ExplosionForce = 1.f;

	/** ShootWeapon's and Shotgun's table: name=set:push:damage[:pellets:between:clip:accuracy:range], comma separated (set the reaction: PISTOL_WEAPON, SMG_WEAPON, POWERFUL_WEAPON, * the defaults; push N s; damage health; pellets a shotgun's), or the game's weaponinfo.xml. Empty: pistol, smg, rifle, shotgun. */
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	FString Weapons;

	/** The game's balance runs out of steps (a push he can't take up in so many drops him). Off: he keeps stepping (the Playroom's). */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bStepLimit = true;

	/** A burning character reacts (flails, staggers, writhes). Off: he burns but carries on. */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bFireReaction = true;

	/** LearnedGetUp's and LearnedWalk's networks (the release's getup.net and walk.net). */
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString GetUpNet = TEXT("getup.net");
	UPROPERTY(Config, EditAnywhere, Category = "Data")
	FString WalkNet = TEXT("walk.net");

	UFUNCTION()
	static TArray<FString> SizeModes() { return { TEXT("Auto"), TEXT("Real"), TEXT("Homer") }; }

	virtual FName GetCategoryName() const override { return TEXT("Plugins"); }
};
