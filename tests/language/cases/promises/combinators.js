// @tier 3
// @covers L14
// all, allSettled, race and any on fixed inputs; results keep input order and rejections are scenario-created.
export async function run() {
  const ok = (v) => Promise.resolve(v)
  const bad = (r) => Promise.reject(new Error(r))
  trace('all', await Promise.all([ok(1), 2, ok('three')]))
  trace('settled', await Promise.allSettled([ok('x'), bad('nope')]).then((rs) => rs.map((r) => r.status)))
  trace('race', await Promise.race([ok('fast'), ok('slow')]))
  trace('any', await Promise.any([bad('a'), ok('b')]))
  const failed = await Promise.all([ok(1), bad('boom')]).catch((e) => e.message)
  trace('all-rejects', failed)
}
