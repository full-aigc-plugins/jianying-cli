/// 已迁移的剪映资源语义类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftResourceKind {
    Video,
    Audio,
    Text,
    Sticker,
    Filter,
    Effect,
    Transition,
    Mask,
    Animation,
    Keyframe,
}
