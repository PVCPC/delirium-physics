pub fn compile_active<T, R>(
    registered: impl IntoIterator<Item = T>,
    endpoint_states: impl Fn(&T) -> [u32; 2],
    mut compile: impl FnMut(T) -> R,
) -> Vec<R> {
    registered
        .into_iter()
        .filter(|constraint| {
            let [a, b] = endpoint_states(constraint);
            (a | b) & 4 != 0
        })
        .map(&mut compile)
        .collect()
}
