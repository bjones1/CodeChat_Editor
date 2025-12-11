// `assert.mts`
// ============
//
// Provide a simple `assert` function to check conditions at runtime. Using
// things like [assert](https://nodejs.org/api/assert.html) causes problems --
// somehow, this indicates that the code is running in a development environment
// (see
// [this](https://github.com/micromark/micromark/issues/87#issuecomment-908924233)).
// Taken from the TypeScript
// [docs](https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#assertion-functions).
export function assert(condition: boolean, msg?: string): asserts condition {
    if (!condition) {
        throw new Error(msg);
    }
}
