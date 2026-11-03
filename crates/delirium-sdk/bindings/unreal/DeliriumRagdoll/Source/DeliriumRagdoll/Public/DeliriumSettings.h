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

	/** A shot nearest the head kills. */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bHeadshotKills = true;

	/** Ragdolls try to keep their feet (off: they go limp). */
	UPROPERTY(Config, EditAnywhere, Category = "Feel")
	bool bBalance = true;

	/** Every bullet's, blow's and explosion's push times these. */
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float ShotForce = 1.f;
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float MeleeForce = 1.f;
	UPROPERTY(Config, EditAnywhere, Category = "Force")
	float ExplosionForce = 1.f;

	UFUNCTION()
	static TArray<FString> SizeModes() { return { TEXT("Auto"), TEXT("Real"), TEXT("Homer") }; }

	virtual FName GetCategoryName() const override { return TEXT("Plugins"); }
};
