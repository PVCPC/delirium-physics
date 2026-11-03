using System.IO;
using UnrealBuildTool;

public class DeliriumRagdoll : ModuleRules
{
	public DeliriumRagdoll(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(new[] { "Core", "CoreUObject", "Engine", "DeveloperSettings", "Chaos", "PhysicsCore", "Projects", "InputCore" });
		// The SDK: delirium_physics.h, delirium_physics.dll and delirium_physics.dll.lib in Source/ThirdParty/delirium_physics (from the release, or rust/target/release).
		string Sdk = Path.Combine(ModuleDirectory, "..", "ThirdParty", "delirium_physics");
		PublicIncludePaths.Add(Sdk);
		PublicAdditionalLibraries.Add(Path.Combine(Sdk, "delirium_physics.dll.lib"));
		PublicDelayLoadDLLs.Add("delirium_physics.dll");
		RuntimeDependencies.Add("$(BinaryOutputDir)/delirium_physics.dll", Path.Combine(Sdk, "delirium_physics.dll"));
	}
}
