//! vn-runtime::shapes: PatchFlags, ShapeFlags and SlotFlags with the numeric values of @vue/shared 3.5.43 (the values are
//! checked against the installed package in tests/ui/runtime-a/corpora/flags.json; the compiler emits them as numbers).

/// `PatchFlags` of @vue/shared. CACHED and BAIL are the negative markers of hoisted and bailed vnodes.
pub struct PatchFlags;

impl PatchFlags {
    pub const TEXT: i32 = 1;
    pub const CLASS: i32 = 2;
    pub const STYLE: i32 = 4;
    pub const PROPS: i32 = 8;
    pub const FULL_PROPS: i32 = 16;
    pub const NEED_HYDRATION: i32 = 32;
    pub const STABLE_FRAGMENT: i32 = 64;
    pub const KEYED_FRAGMENT: i32 = 128;
    pub const UNKEYED_FRAGMENT: i32 = 256;
    pub const NEED_PATCH: i32 = 512;
    pub const DYNAMIC_SLOTS: i32 = 1024;
    pub const DEV_ROOT_FRAGMENT: i32 = 2048;
    pub const CACHED: i32 = -1;
    pub const BAIL: i32 = -2;
}

/// `ShapeFlags` of @vue/shared: bit set of what a vnode's type and children are.
pub struct ShapeFlags;

impl ShapeFlags {
    pub const ELEMENT: i32 = 1;
    pub const FUNCTIONAL_COMPONENT: i32 = 2;
    pub const STATEFUL_COMPONENT: i32 = 4;
    pub const TEXT_CHILDREN: i32 = 8;
    pub const ARRAY_CHILDREN: i32 = 16;
    pub const SLOTS_CHILDREN: i32 = 32;
    pub const TELEPORT: i32 = 64;
    pub const SUSPENSE: i32 = 128;
    pub const COMPONENT_SHOULD_KEEP_ALIVE: i32 = 256;
    pub const COMPONENT_KEPT_ALIVE: i32 = 512;
    pub const COMPONENT: i32 = 6;
}

/// `SlotFlags` of @vue/shared (the `_` marker of compiled slot objects).
pub struct SlotFlags;

impl SlotFlags {
    pub const STABLE: i32 = 1;
    pub const DYNAMIC: i32 = 2;
    pub const FORWARDED: i32 = 3;
}
