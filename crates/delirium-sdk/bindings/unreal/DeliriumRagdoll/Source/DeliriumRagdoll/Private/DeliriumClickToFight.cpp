#include "DeliriumClickToFight.h"
#include "DeliriumRagdollComponent.h"
#include "Components/InputComponent.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "InputCoreTypes.h"
#include "UObject/UObjectIterator.h"

void UDeliriumClickToFight::BeginPlay()
{
	Super::BeginPlay();
	for (UDeliriumRagdollComponent* D : Ragdolls())
		if (D->GetUpAfter <= 0.f) D->GetUpAfter = 3.f;
	APlayerController* PC = GetWorld()->GetFirstPlayerController();
	AActor* Owner = GetOwner();
	if (!PC || !Owner) return;
	Owner->EnableInput(PC);
	if (UInputComponent* In = Owner->InputComponent)
	{
		In->BindKey(EKeys::LeftMouseButton, IE_Pressed, this, &UDeliriumClickToFight::OnFire);
		In->BindKey(EKeys::F, IE_Pressed, this, &UDeliriumClickToFight::OnPunch);
		In->BindKey(EKeys::G, IE_Pressed, this, &UDeliriumClickToFight::OnBlast);
	}
}

bool UDeliriumClickToFight::View(FVector& From, FVector& Aim) const
{
	APlayerController* PC = GetWorld()->GetFirstPlayerController();
	if (!PC) return false;
	FRotator Rot;
	PC->GetPlayerViewPoint(From, Rot);
	Aim = Rot.Vector();
	return true;
}

void UDeliriumClickToFight::OnFire() { FVector F, A; if (View(F, A)) Fire(F, A); }
void UDeliriumClickToFight::OnPunch() { FVector F, A; if (View(F, A)) Punch(F, A); }
void UDeliriumClickToFight::OnBlast() { FVector F, A; if (View(F, A)) Blast(F, A); }

bool UDeliriumClickToFight::Fire(FVector From, FVector Aim)
{
	FVector Chest;
	UDeliriumRagdollComponent* D = Aimed(From, Aim, Chest);
	if (!D) return false;
	float& Hp = Health.FindOrAdd(D, 200.f);
	Hp = FMath::Max(0.f, Hp - ShotDamage);
	return D->ShootAlong(From, Aim, 40.f, TEXT("POWERFUL_WEAPON"), Hp);
}

bool UDeliriumClickToFight::Punch(FVector From, FVector Aim)
{
	FVector Chest;
	UDeliriumRagdollComponent* D = Aimed(From, Aim, Chest);
	if (!D || FVector::Dist(From, Chest) > 300.f) return false;
	return D->MeleeHit(Chest, Aim.GetSafeNormal() * PunchImpulse, From);
}

int32 UDeliriumClickToFight::Blast(FVector From, FVector Aim)
{
	Aim = Aim.GetSafeNormal();
	FVector At = From + Aim * 1000.f;
	FHitResult Hit;
	FCollisionQueryParams Q;
	Q.AddIgnoredActor(GetOwner());
	if (GetWorld()->LineTraceSingleByChannel(Hit, From, From + Aim * 10000.f, ECC_Visibility, Q)) At = Hit.ImpactPoint;
	int32 N = 0;
	for (UDeliriumRagdollComponent* D : Ragdolls())
		if (FVector::Dist(ChestOf(D), At) < BlastRadius && D->Explode(At, BlastRadius)) N++;
	return N;
}

UDeliriumRagdollComponent* UDeliriumClickToFight::Aimed(const FVector& From, FVector Aim, FVector& Chest) const
{
	// The character whose chest is nearest the aim line (within 1.5 m of it).
	UDeliriumRagdollComponent* Best = nullptr;
	float BestOff = 150.f;
	Aim = Aim.GetSafeNormal();
	for (UDeliriumRagdollComponent* D : Ragdolls())
	{
		const FVector C = ChestOf(D), To = C - From;
		const float Off = FVector::CrossProduct(Aim, To).Size();
		if (FVector::DotProduct(To, Aim) > 0 && Off < BestOff) { Best = D; BestOff = Off; Chest = C; }
	}
	return Best;
}

TArray<UDeliriumRagdollComponent*> UDeliriumClickToFight::Ragdolls() const
{
	TArray<UDeliriumRagdollComponent*> Out;
	for (TObjectIterator<UDeliriumRagdollComponent> It; It; ++It)
		if (It->GetWorld() == GetWorld() && IsValid(*It) && It->GetOwner()) Out.Add(*It);
	return Out;
}

FVector UDeliriumClickToFight::ChestOf(const UDeliriumRagdollComponent* D)
{
	// Its hips while ragdolled (the body's moved off its actor), else the middle of the actor's bounds.
	return D->IsRagdolled() ? D->GetHips() : D->GetOwner()->GetComponentsBoundingBox().GetCenter();
}
