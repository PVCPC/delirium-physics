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
var ground: StaticBody3D

func expect(what: String, ok: bool) -> void:
	print(("ok   " if ok else "FAIL ") + what)
	if not ok: failed += 1

func run(what: String, hit: Callable, setup := Callable()) -> float:
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
	if setup.is_valid(): setup.call(d)
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
	ground = floor
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
	await run("tased 2.5 s", func(d): expect("tased", d.tase(2.5) and d.get_state().tased))
	# The body setting and overrides (the Playroom's bodies and Parameters tab).
	expect("Brawler stands", await run("Brawler", func(d): pass, func(d): d.body = "Brawler") > 0.5)
	expect("Random stands", await run("Random seed 5", func(d): pass, func(d): d.body = "Random"; d.body_seed = 5) > 0.5)
	expect("Custom heavy stands", await run("Custom", func(d): pass, func(d): d.body = "Custom"; d.custom_body = "1, 1, 1, 1.2, 1, 1, 95, 0") > 0.5)
	expect("bone_map stands", await run("bone_map", func(d): pass, func(d): d.bone_map = "NoSuchBone=Head, Tail=None, Hips=Bogus") > 0.5)
	expect("bone_map maps (no hips: no ragdoll)", await run("bone_map none", func(d): pass, func(d): d.bone_map = "Hips=None, LeftUpLeg=None, RightUpLeg=None") == 0.0)
	expect("overrides", await run("override", func(d):
		expect("good override", d.set_override("bodyBalance", "useHeadLook", 0))
		expect("bad override refused", not d.set_override("bodyBalance", "noSuchThing", 1)),
		func(d): d.overrides = "shot.bodyStiffness=7") > 0.5)
	# State and events.
	var seen := {"fell": 0}
	expect("balancing while standing", await run("state standing", func(d):
		var st: Dictionary = d.get_state()
		expect("state: ragdolled and balancing", st.ragdolled and st.balancing and not st.fallen and st.mass > 40.0)) > 0.7)
	expect("fallen after a hard push", await run("state fallen", func(d):
		d.fell.connect(func(): seen.fell += 1)
		d.push(Vector3(200, 0, 0))) < 0.4)
	expect("fell raised once", seen.fell == 1)
	# Movers: a car-sized box driven through him at 8 m/s, then the same as an object (the game's damage per kind).
	var hurt := {}
	for kind in [0, 1]:
		var car := Node3D.new()
		get_root().add_child(car)
		var name := "car" if kind == 0 else "object"
		var moved := {}
		await run(name + " hit", func(d):
			var from: Vector3 = d.get_hips()
			car.position = from + Vector3(-3.5, -0.25, 0)
			get_root().get_tree().create_timer(1.5).timeout.connect(func(): moved[0] = d.get_hips().x - from.x)
			d.register_mover(car, kind, Vector3(4.6, 1.4, 1.8), false, true, true, Vector3.ZERO)
			d.car_hit.connect(func(dmg, c): hurt[name] = dmg)
			car.create_tween().tween_property(car, "position", car.position + Vector3(9, 0, 0), 9.0 / 8.0))
		# As the SDK's boxes_and_ice: an 8 m/s box shoves him along (over 1 m).
		expect("%s shoves him along (%.2f m)" % [name, moved.get(0, 0.0)], moved.get(0, 0.0) > 1.0)
		expect("%s hit reported" % name, hurt.has(name) and hurt[name] > 0.0)
		ClassDB.instantiate("DeliriumRagdoll").unregister_car(car)
		car.queue_free()
	print("damage: ", hurt)
	expect("an object hurts less than a car", hurt.get("object", 999.0) < hurt.get("car", 0.0))
	# The game's tasks.
	expect("catch fall", await run("catch fall", func(d): expect("catch_fall", d.catch_fall()); d.push(Vector3(150, 0, 0))) < 0.5)
	await run("jump roll", func(d): expect("jump_roll", d.jump_roll()))
	await run("flinch", func(d): expect("flinch", d.flinch(d.get_hips() + Vector3(1, 0.6, 0))))
	await run("react", func(d): expect("react", d.react("bodyRollUp start=true")))
	# Surfaces: the floor's physics material (ice, friction 0.05) under a shove's feet: he slides further.
	var slid := {}
	for f in [1.0, 0.05]:
		var m := PhysicsMaterial.new()
		m.friction = f
		ground.physics_material_override = m
		await run("friction %.2f" % f, func(d):
			var from: Vector3 = d.get_hips()
			get_root().get_tree().create_timer(2.0).timeout.connect(func(): slid[f] = (d.get_hips() - from).length())
			d.push(Vector3(120, 0, 0)))
	ground.physics_material_override = null
	print("slid: ", slid)
	expect("ice slides further", slid.get(0.05, 0.0) > slid.get(1.0, 0.0) + 0.05)
	# The Playroom's other tasks and reads (the SDK's parity calls).
	var R = ClassDB.instantiate("DeliriumRagdoll")
	expect("weapons: the default four", R.weapon_names().size() == 4)
	var msgs: Array = R.messages()
	expect("messages listed (%d)" % msgs.size(), msgs.size() > 50 and msgs[0].params.size() > 0)
	R.free()
	await run("weapon shot", func(d): expect("pistol leaves 175", is_equal_approx(d.shoot_weapon("pistol", d.get_hips() + Vector3(0, 0.4, 0), Vector3(1, 0, 0), 200.0), 175.0)))
	await run("shotgun", func(d): expect("pellets strike", d.shotgun("shotgun", d.get_hips() + Vector3(-3, 0.3, 0), Vector3(1, 0, 0), 150.0, 7) > 0))
	await run("state reads", func(d):
		var st: Dictionary = d.get_state()
		expect("stance while balancing", st.has("stance") and st.contacts.size() == 21 and st.awake)
		expect("raycast hits a part", d.raycast(d.get_hips() + Vector3(-2, 0.3, 0), d.get_hips() + Vector3(2, 0.3, 0)).has("part"))
		expect("get_param", d.get_param("bodyBalance", "useHeadLook") != null)
		expect("check_setup clean", d.check_setup() == "")
		expect("balance, stop", d.balance(Vector3.ZERO) and d.stop()))
	await run("reactions", func(d):
		expect("set_reactions", d.set_reactions(d.REACT_WINDMILL | d.REACT_DODGE | d.REACT_LOOK_HIT))
		expect("set_dive", d.set_dive(3.5))
		expect("near_miss", d.near_miss(d.get_hips() + Vector3(-5, 0.5, 0.3), Vector3(1, 0, 0)))
		expect("thrown_at", d.thrown_at(d.get_hips() + Vector3(2, 1, 0), Vector3(-5, 0, 0), true, true)))
	await run("jump", func(d): expect("jump", d.jump(false, Vector3(0, 0, 1)) and d.get_state().jumping))
	await run("sit", func(d): expect("sit", d.sit(Vector3.ZERO, Vector3.ZERO) and d.get_state().task_running))
	await run("pose", func(d): expect("pose", d.pose(4, 6)))
	await run("roll up relax", func(d): expect("roll_up_relax", d.roll_up_relax(-1, -1, 1000)))
	await run("script", func(d): expect("script", d.script(500)))
	await run("muscles", func(d): expect("muscle_rise", d.muscle_rise(true, -1) and d.muscle_rise(false, 0)))
	await run("stick", func(d): expect("set_stick", d.set_stick(true, 0, 1, 0)))
	await run("strength", func(d): d.set_strength(1.8); expect("set_strength", is_equal_approx(d.strength, 1.8)); d.strength = 0)
	var nets := OS.get_environment("DELIRIUM_NETS")
	if nets != "":
		await run("learned get-up", func(d):
			d.push(Vector3(200, 0, 0))
			expect("learned_getup", d.learned_getup(nets + "/getup-reward.net") and d.get_state().learned_getup))
		await run("learned walk", func(d): expect("learned_walk", d.learned_walk(nets + "/walk.net", 1.0) and d.set_stick(true, 0, 1, 0)))
	# engine_physics: NM's own physics moves him, his parts as Godot bodies pushing Godot's; loose bodies hit him.
	ProjectSettings.set_setting("delirium/engine_physics", true)
	expect("engine: stands", await run("engine standing 5 s", func(d): pass) > 0.7)
	expect("engine: stays up after a push", await run("engine 60 N s push", func(d): d.push(Vector3(60, 0, 0))) > 0.7)
	expect("engine: down after a hard push", await run("engine 200 N s push", func(d): d.push(Vector3(200, 0, 0))) < 0.4)
	expect("engine: down after a killing shot", await run("engine shot", func(d): d.shoot(d.get_hips() + Vector3(0, 0.4, 0), Vector3(1, 0, 0), 40.0, "POWERFUL_WEAPON", 40.0)) < 0.4)
	expect("engine: down without a leg", await run("engine leg cut", func(d): d.sever(d.get_hips() + Vector3(0.1, -0.6, 0), Vector3(0, 0.2, 0))) < 0.4)
	for b in ["Brute", "Brawler", "Bruiser"]:
		expect("engine: %s stands" % b, await run("engine %s" % b, func(d): pass, func(d): d.body = b) > 0.5)
	expect("engine: Random stands", await run("engine Random seed 5", func(d): pass, func(d): d.body = "Random"; d.body_seed = 5) > 0.5)
	ProjectSettings.set_setting("delirium/engine_physics", false)
	print("DONE failed=%d" % failed)
	quit(failed)
