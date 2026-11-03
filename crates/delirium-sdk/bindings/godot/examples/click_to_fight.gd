# Click to fight: attach to your Camera3D and aim at a character with a DeliriumRagdoll node.
# Left click shoots (he staggers or falls; the fourth shot kills), F punches (within 3 m), G throws a blast where you aim.
# Downed characters get up after 3 s. No colliders needed on the characters.
extends Camera3D

@export var shot_damage := 40.0
@export var punch := 30.0
@export var blast_radius := 5.0

var health := {}

func _ready() -> void:
	for d in ragdolls():
		if d.get_up_after <= 0.0:
			d.get_up_after = 3.0

func _unhandled_input(event: InputEvent) -> void:
	var aim := -global_basis.z
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		fire(global_position, aim)
	elif event is InputEventKey and event.pressed and not event.echo:
		if event.keycode == KEY_F: punch_at(global_position, aim)
		elif event.keycode == KEY_G: blast(global_position, aim)

## Shoots the character nearest the aim; its health drops by shot_damage (200 to start).
func fire(from: Vector3, aim: Vector3) -> bool:
	var d = aimed(from, aim)
	if d == null: return false
	health[d] = maxf(0.0, health.get(d, 200.0) - shot_damage)
	return d.shoot_along(from, aim, 40.0, "POWERFUL_WEAPON", health[d])

## Punches the character nearest the aim, when it's within 3 m.
func punch_at(from: Vector3, aim: Vector3) -> bool:
	var d = aimed(from, aim)
	if d == null or from.distance_to(chest(d)) > 3.0: return false
	return d.melee_hit(chest(d), aim.normalized() * punch, from, false)

## A blast where the aim meets the level (10 m ahead if it meets nothing), throwing everyone in range.
func blast(from: Vector3, aim: Vector3) -> int:
	var at := from + aim.normalized() * 10.0
	var hit := get_world_3d().direct_space_state.intersect_ray(PhysicsRayQueryParameters3D.create(from, from + aim.normalized() * 100.0))
	if hit: at = hit.position
	var n := 0
	for d in ragdolls():
		if chest(d).distance_to(at) < blast_radius and d.explode(at, blast_radius, 15.0): n += 1
	return n

## The character whose chest is nearest the aim line (within 1.5 m of it).
func aimed(from: Vector3, aim: Vector3):
	var best = null
	var best_off := 1.5
	aim = aim.normalized()
	for d in ragdolls():
		var to := chest(d) - from
		var off := aim.cross(to).length()
		if to.dot(aim) > 0.0 and off < best_off:
			best = d
			best_off = off
	return best

## Its hips while ragdolled (the body's moved off its root), else a metre above its root.
func chest(d) -> Vector3:
	if d.is_ragdolled(): return d.get_hips()
	var root: Node3D = d.character if d.character else d.get_parent()
	return root.global_position + Vector3.UP

func ragdolls() -> Array:
	return get_tree().root.find_children("*", "DeliriumRagdoll", true, false)
