#pragma once

#include "Components/ActorComponent.h"
#include "DeliriumClickToFight.generated.h"

class UDeliriumRagdollComponent;

/**
 * Click to fight: add to your player pawn and aim at characters with a Delirium Ragdoll component.
 * Left click shoots (he staggers or falls; the fourth shot kills), F punches (within 3 m), G throws a blast where you aim.
 * Downed characters get up after 3 s. Fire, Punch and Blast are Blueprint-callable too, from any view.
 */
UCLASS(ClassGroup = Physics, meta = (BlueprintSpawnableComponent))
class DELIRIUMRAGDOLL_API UDeliriumClickToFight : public UActorComponent
{
	GENERATED_BODY()

public:
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float ShotDamage = 40.f;

	/** The punch's impulse, N s. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float PunchImpulse = 30.f;

	/** cm. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Delirium")
	float BlastRadius = 500.f;

	/** Shoots the character nearest the aim; its health drops by ShotDamage (200 to start). */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Fire(FVector From, FVector Aim);

	/** Punches the character nearest the aim, when it's within 3 m. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	bool Punch(FVector From, FVector Aim);

	/** A blast where the aim meets the level (10 m ahead if it meets nothing), throwing everyone in range. */
	UFUNCTION(BlueprintCallable, Category = "Delirium")
	int32 Blast(FVector From, FVector Aim);

	virtual void BeginPlay() override;

private:
	void OnFire();
	void OnPunch();
	void OnBlast();
	bool View(FVector& From, FVector& Aim) const;
	UDeliriumRagdollComponent* Aimed(const FVector& From, FVector Aim, FVector& Chest) const;
	TArray<UDeliriumRagdollComponent*> Ragdolls() const;
	static FVector ChestOf(const UDeliriumRagdollComponent* D);
	TMap<TWeakObjectPtr<UDeliriumRagdollComponent>, float> Health;
};
