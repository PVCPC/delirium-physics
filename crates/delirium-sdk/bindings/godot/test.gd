# Headless: godot --headless --fixed-fps 60 --path . --script test.gd
extends SceneTree

const BONES = [
	["Hips", "", Vector3(0, 1.0, 0)], ["Spine", "Hips", Vector3(0, 1.1, 0)], ["Spine1", "Spine", Vector3(0, 1.25, 0)],
	["Spine2", "Spine1", Vector3(0, 1.4, 0)], ["Neck", "Spine2", Vector3(0, 1.55, 0)], ["Head", "Neck", Vector3(0, 1.65, 0)],
	["LeftShoulder", "Spine2", Vector3(0.05, 1.5, 0)], ["LeftArm", "LeftShoulder", Vector3(0.18, 1.5, 0)],
	["LeftForeArm", "LeftArm", Vector3(0.45, 1.5, 0)], ["LeftHand", "LeftForeArm", Vector3(0.7, 1.5, 0)],
	["RightShoulder", "Spine2", Vector3(-0.05, 1.5, 0)], ["RightArm", "RightShoulder", Vector3(-0.18, 1.5, 0)],
	["RightForeArm", "RightArm", Vector3(-0.45, 1.5, 0)], ["RightHand", "RightForeArm", Vector3(-0.7, 1.5, 0)],
	["LeftUpLeg", "Hips", Vector3(0.1, 0.95, 0)], ["LeftLeg", "LeftUpLeg", Vector3(0.1, 0.5, 0)],
	["LeftFoot", "LeftLeg", Vector3(0.1, 0.08, 0)], ["LeftToeBase", "LeftFoot", Vector3(0.1, 0.02, 0.12)],
	["RightUpLeg", "Hips", Vector3(-0.1, 0.95, 0)], ["RightLeg", "RightUpLeg", Vector3(-0.1, 0.5, 0)],
	["RightFoot", "RightLeg", Vector3(-0.1, 0.08, 0)], ["RightToeBase", "RightFoot", Vector3(-0.1, 0.02, 0.12)],
]
var failed := 0

func expect(what: String, ok: bool) -> void:
	print(("ok   " if ok else "FAIL ") + what)
	if not ok: failed += 1

func run(what: String, hit: Callable) -> float:
	var root := Node3D.new()
	get_root().add_child(root)
	var sk := Skeleton3D.new()
	root.add_child(sk)
	for b in BONES:
		var i := sk.add_bone(b[0])
		var at: Vector3 = b[2]
		if b[1] != "":
			var p := sk.find_bone(b[1])
			sk.set_bone_parent(i, p)
			at -= BONES[p][2]
		sk.set_bone_rest(i, Transform3D(Basis(), at))
		sk.set_bone_pose_position(i, at)
	var d = ClassDB.instantiate("DeliriumRagdoll")
	sk.add_child(d)
	d.character = root
	await process_frame
	var hips := 0.0
	if d.ragdoll():
		for f in 60: await process_frame
		hit.call(d)
		for f in 300: await process_frame
		hips = d.get_hips().y
		d.get_up()
		for f in 30: await process_frame
		expect(what + ": handed back", not d.is_ragdolled())
	print("%s: hips at %.2f m" % [what, hips])
	root.queue_free()
	await process_frame
	return hips

func _initialize() -> void:
	var floor := StaticBody3D.new()
	var cs := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(20, 1, 20)
	cs.shape = box
	if OS.get_environment("FLOOR") == "mesh":
		var m := BoxMesh.new()
		m.size = box.size
		cs.shape = m.create_trimesh_shape()
	floor.add_child(cs)
	floor.position = Vector3(0, -0.5, 0)
	get_root().add_child.call_deferred(floor)
	go.call_deferred()

func go() -> void:
	await physics_frame
	await physics_frame
	expect("stands", await run("standing 5 s", func(d): pass) > 0.7)
	expect("stays up after a push", await run("60 N s push", func(d): d.push(Vector3(60, 0, 0))) > 0.7)
	expect("down after a hard push", await run("200 N s push", func(d): d.push(Vector3(200, 0, 0))) < 0.4)
	expect("down after a killing shot", await run("shot, health 40", func(d): d.shoot(d.get_hips() + Vector3(0, 0.4, 0), Vector3(1, 0, 0), 40.0, "POWERFUL_WEAPON", 40.0)) < 0.4)
	expect("down without a leg", await run("leg cut", func(d): expect("cut", d.sever(d.get_hips() + Vector3(0.1, -0.6, 0), Vector3(0, 0.2, 0)))) < 0.4)
	print("DONE failed=%d" % failed)
	quit(failed)
