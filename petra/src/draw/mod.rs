//! The draw list: the closed, versioned command stream a `canvas` node draws.
//!
//! `specs/005-petra-carbon-authoring/contracts/draw-list.md` is binding here.
//! Where this module and that contract disagree, the contract wins.
//!
//! # What this is, and what it is not
//!
//! A [`DrawList`] is a **flat** `Vec<Command>` with an explicit state stack,
//! never a tree. Six commands, no more (§1). Everything an author might reach
//! for that is not here — text, arcs, gradients, nested groups, blend modes,
//! meshes, path modifiers — is excluded by name in §2 with the reason, and
//! adding one is a `v1 -> v2` change that bumps this module's [`VERSION`] and
//! the two digest domains with it.
//!
//! It is also **not** a hosted payload. Every coordinate below reaches the
//! frame digest through [`crate::frame::digest::hash_paint_content`], which is
//! the whole reason a canvas is worth having over a registered custom painter:
//! a `custom` node's picture is a name in the digest, and a canvas's picture
//! is the picture.
//!
//! # Refusal, not degradation
//!
//! Three things are refused at construction rather than papered over at paint
//! time, because all three produce a *wrong picture* under a digest that says
//! the frame is right:
//!
//! * over budget ([`MAX_DRAW_COMMANDS`], [`MAX_DRAW_PATH_VERBS`]) — truncating
//!   at paint time and reporting success is the failure mode `PaintReport`
//!   exists to catch, not to launder (§5);
//! * a non-convex filled [`Command::Path`] — `epaint` fills convex polygons
//!   only, so this draws *wrong*, not merely slow (§5);
//! * an unbalanced `Push`/`Pop` pair, which leaves a transform or a clip in
//!   force over commands the author never meant it to reach.
//!
//! # No clock, ever
//!
//! No command field is a clock, a frame counter, or anything else derived from
//! time (§8). A list whose content is a function of wall-clock time while the
//! digest sees only the unevaluated expression makes two identical digests
//! draw two different pictures. A canvas whose content really does change
//! every frame — a live plot — rebuilds and resubmits a whole new list, pays
//! one digest change per frame, and declares `ambient`. The test that holds
//! this module to it is `the_draw_list_takes_no_time_input`, in
//! `crate::anim::registry`, beside the engine-side loop rule that makes the
//! obligation unnecessary.

use serde::{Deserialize, Serialize};

use crate::geom::{Point, Rect, Size};

mod arc;

pub use arc::arc_verbs;

/// The wire version of the command set.
///
/// A list that names any other version is refused whole. Growing the set — a
/// seventh command, a new field on an existing one — moves this to `v2` and
/// moves [`crate::frame::digest::PAINT_DOMAIN`] and
/// [`crate::frame::digest::DOMAIN`] with it, because the payload stream is
/// then a different stream (`contracts/frame-identity.md`, "Changing the
/// stream").
pub const VERSION: &str = "drawlist-v1";

/// How many commands one canvas may carry.
///
/// Kin to [`crate::component::MAX_LAYER_DEPTH`] — a bound on what an author
/// may ask the engine to do in one node — and here rather than beside it
/// because that constant belongs to the component vocabulary and this one
/// belongs to the draw list. Unbenchmarked, and `contracts/draw-list.md`
/// "Open" says so; what is not open is that the bound is enforced at
/// construction.
pub const MAX_DRAW_COMMANDS: usize = 4096;

/// How many path verbs one canvas may carry, summed over every
/// [`Command::Path`] in the list.
///
/// Separate from [`MAX_DRAW_COMMANDS`] because the two costs are different:
/// a command is a dispatch, and a verb is tessellation work. One `Path` of
/// twenty thousand verbs is one command and is refused here.
pub const MAX_DRAW_PATH_VERBS: usize = 16_384;

/// A colour a command paints with.
///
/// Resolved through the same `TokenSource` a hosted painter reads, so a
/// canvas sits on the design system rather than beside it. The literal form
/// exists for the one case a token cannot express — a data colour an
/// application computed — and is hashed by value like everything else.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ColorRef {
    /// A theme token name, resolved at paint time.
    Token(String),
    /// A literal colour, `[r, g, b, a]`, unmultiplied.
    Rgba([u8; 4]),
}

/// A stroke width, in the units the author chose to state it in.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Width {
    /// Logical units. Scales with the frame, like every other coordinate.
    Logical(f32),
    /// Device pixels. Resolved as `n / scale.factor()` logical units **at
    /// execution time**, so a scale change moves the stroke without moving
    /// the list — and therefore without moving the digest, which is right:
    /// the author declared a one-pixel hairline and still has one.
    Device(f32),
}

impl Width {
    /// This width in logical units at `scale`.
    #[must_use]
    pub fn logical(self, scale: crate::geom::Scale) -> f32 {
        match self {
            Self::Logical(units) => units,
            Self::Device(pixels) => pixels / scale.factor(),
        }
    }
}

/// A stroke: how wide, and in what colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    /// Line width.
    pub width: Width,
    /// Line colour.
    pub color: ColorRef,
}

/// What one shape is filled and outlined with.
///
/// Both members optional, and both absent is legal: a shape that paints
/// nothing is a shape the interpreter reports as drawing nothing, which is
/// the honest outcome. Refusing it here would make a list that an author
/// builds incrementally — fill first, stroke later — un-constructible
/// mid-edit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Paint {
    /// Interior colour, or `None` for an unfilled shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<ColorRef>,
    /// Outline, or `None` for an unstroked shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

impl Paint {
    /// A filled shape with no outline.
    #[must_use]
    pub fn filled(fill: ColorRef) -> Self {
        Self {
            fill: Some(fill),
            stroke: None,
        }
    }

    /// An outlined shape with no fill.
    #[must_use]
    pub fn stroked(stroke: Stroke) -> Self {
        Self {
            fill: None,
            stroke: Some(stroke),
        }
    }
}

/// Four corner radii, in logical units, clockwise from the top left.
///
/// Four rather than one because `epaint`'s own `CornerRadius` is four, so one
/// shared radius is representable here and four are not representable there —
/// the direction that loses nothing. `contracts/draw-list.md` "Open" leaves
/// the final call to FR-022; whichever way it lands, [`Corners::all`] is the
/// one-radius spelling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Corners {
    /// Top-left radius.
    pub top_left: f32,
    /// Top-right radius.
    pub top_right: f32,
    /// Bottom-right radius.
    pub bottom_right: f32,
    /// Bottom-left radius.
    pub bottom_left: f32,
}

impl Corners {
    /// Square corners.
    pub const SQUARE: Self = Self::all(0.0);

    /// One radius on all four corners.
    #[must_use]
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

/// A translate-and-scale transform.
///
/// **No rotation and no skew** (§1). Not an oversight: the digest hashes what
/// the author wrote, and the four numbers below compose associatively under
/// the flat `Push`/`Pop` stack without a matrix product. A rotation would need
/// one, and the first thing a matrix product costs is the guarantee that two
/// runs of the same list on two targets compose to the same bits.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Affine {
    /// Horizontal translation, logical units.
    pub tx: f32,
    /// Vertical translation, logical units.
    pub ty: f32,
    /// Horizontal scale factor.
    pub sx: f32,
    /// Vertical scale factor.
    pub sy: f32,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    /// The transform that moves and scales nothing.
    pub const IDENTITY: Self = Self {
        tx: 0.0,
        ty: 0.0,
        sx: 1.0,
        sy: 1.0,
    };

    /// A pure translation.
    #[must_use]
    pub const fn translate(tx: f32, ty: f32) -> Self {
        Self {
            tx,
            ty,
            sx: 1.0,
            sy: 1.0,
        }
    }

    /// `self` applied after `outer`: the transform an interpreter holds after
    /// pushing `self` onto a stack whose top was `outer`.
    ///
    /// Scale-then-translate in that order, which is what makes composition
    /// associative for this restricted family and is why the family is
    /// restricted.
    #[must_use]
    pub fn then(self, outer: Self) -> Self {
        Self {
            tx: outer.tx + self.tx * outer.sx,
            ty: outer.ty + self.ty * outer.sy,
            sx: outer.sx * self.sx,
            sy: outer.sy * self.sy,
        }
    }

    /// `point` mapped through this transform.
    #[must_use]
    pub fn apply(self, point: Point) -> Point {
        Point::new(point.x * self.sx + self.tx, point.y * self.sy + self.ty)
    }

    /// `rect` mapped through this transform.
    #[must_use]
    pub fn apply_rect(self, rect: Rect) -> Rect {
        Rect::new(
            rect.x * self.sx + self.tx,
            rect.y * self.sy + self.ty,
            rect.w * self.sx,
            rect.h * self.sy,
        )
    }
}

/// How a sprite fills the rect it is drawn into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// Stretch to the destination rect, ignoring the source aspect ratio.
    #[default]
    Fill,
    /// Scale uniformly until one axis fits, and centre on the other.
    Contain,
}

impl Fit {
    /// The fit's wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Contain => "contain",
        }
    }
}

/// One step of a path.
///
/// No arc verb: `epaint` has no arc shape, so an arc would have to be
/// converted to cubics somewhere, and doing it at authoring time keeps the
/// verb count — which [`MAX_DRAW_PATH_VERBS`] bounds — a number the author
/// can see (§2). [`arc_verbs`] is that conversion.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum PathVerb {
    /// Start a new sub-path at this point.
    MoveTo(Point),
    /// Straight segment to this point.
    LineTo(Point),
    /// Quadratic segment.
    QuadTo {
        /// The single control point.
        ctrl: Point,
        /// The segment's end point.
        to: Point,
    },
    /// Cubic segment.
    CubicTo {
        /// The control point leaving the current point.
        c1: Point,
        /// The control point entering `to`.
        c2: Point,
        /// The segment's end point.
        to: Point,
    },
    /// Close the current sub-path back to its start.
    Close,
}

impl PathVerb {
    /// Every point this verb contributes to the control polygon, in the order
    /// the path visits them.
    ///
    /// Control points included, which is what makes the convexity test in
    /// [`DrawList::new`] sound: a bézier segment lies inside the convex hull
    /// of its own control points, so a control polygon that is convex and
    /// turns one way bounds a convex region.
    #[must_use]
    pub fn points(self) -> Vec<Point> {
        match self {
            Self::MoveTo(p) | Self::LineTo(p) => vec![p],
            Self::QuadTo { ctrl, to } => vec![ctrl, to],
            Self::CubicTo { c1, c2, to } => vec![c1, c2, to],
            Self::Close => Vec::new(),
        }
    }

    /// The verb's wire name, for the digest stream and for a message.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MoveTo(_) => "move-to",
            Self::LineTo(_) => "line-to",
            Self::QuadTo { .. } => "quad-to",
            Self::CubicTo { .. } => "cubic-to",
            Self::Close => "close",
        }
    }
}

/// A reference to a registered asset, as `owner/name`.
///
/// Never a bare string (§9): two plugins that both register `icon.png` are two
/// different pictures, and a bare name gives the registry no way to say so.
/// The host's own images keep working under the reserved owner
/// [`AssetRef::HOST_OWNER`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    /// Who registered the asset.
    pub owner: String,
    /// The asset's name within that owner.
    pub name: String,
}

impl AssetRef {
    /// The owner an existing bare image string resolves under.
    pub const HOST_OWNER: &'static str = "host";

    /// An asset owned by `owner`.
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }

    /// An asset under the reserved host owner.
    pub fn host(name: impl Into<String>) -> Self {
        Self::new(Self::HOST_OWNER, name)
    }

    /// The `owner/name` wire form.
    #[must_use]
    pub fn wire(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// One draw command. The set is exactly these six (§1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    /// Push a transform, a clip and an opacity onto the state stack. Each
    /// member is optional and an absent one leaves that part of the state
    /// alone.
    Push {
        /// Transform composed onto the one in force.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transform: Option<Affine>,
        /// Clip intersected with the one in force, in canvas-local units.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<Rect>,
        /// Opacity multiplied into the one in force.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opacity: Option<f32>,
    },
    /// Pop the state stack. Unbalanced against `Push` is a refusal, never a
    /// silently-ignored command.
    Pop,
    /// An axis-aligned rectangle, optionally rounded.
    Rect {
        /// The rectangle, canvas-local logical units.
        rect: Rect,
        /// Corner radii.
        #[serde(default, skip_serializing_if = "is_square")]
        radius: Corners,
        /// Whether to snap the four edges to the device grid at paint time.
        /// The **only** command that may snap (§3).
        #[serde(default, skip_serializing_if = "is_false")]
        snap: bool,
        /// Fill and stroke.
        paint: Paint,
    },
    /// An axis-aligned ellipse.
    ///
    /// A stroke is **centred on the radii**, half inside and half outside,
    /// as an SVG `<circle>` strokes — the convention every Carbon radius and
    /// stroke width in this crate was measured in. `epaint`'s own ellipse
    /// strokes outside its radius, and a 10-unit track drawn that way at
    /// radius 39 ran to 49, past its 44-unit canvas, and was clipped into
    /// the octagon the operator saw on the loading page. The interpreter
    /// tessellates the ellipse itself and strokes the centreline.
    Ellipse {
        /// Centre, canvas-local logical units.
        center: Point,
        /// Half-extents on the two axes, to the stroke's centreline. A
        /// [`Size`] rather than a fresh two-float type: a pair of
        /// non-negative extents is exactly what `Size` already is, and a
        /// fourth two-float struct in this crate would be one more thing for
        /// a serializer to get subtly different.
        radii: Size,
        /// Fill and stroke.
        paint: Paint,
    },
    /// A path of straight and béziers segments.
    Path {
        /// The verbs, in order.
        verbs: Vec<PathVerb>,
        /// Whether the last sub-path closes back to its start.
        closed: bool,
        /// Fill and stroke. A `fill` on a `closed` path is what triggers the
        /// convexity check in [`DrawList::new`].
        paint: Paint,
    },
    /// A registered asset drawn into a rect.
    Sprite {
        /// Which asset.
        asset: AssetRef,
        /// Where it goes, canvas-local logical units.
        dst: Rect,
        /// Sub-rect of the source, in source pixels, or `None` for all of it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        src: Option<Rect>,
        /// How it fills `dst`.
        #[serde(default, skip_serializing_if = "is_default_fit")]
        fit: Fit,
        /// Multiplied into every texel, or `None` to draw the asset as it is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tint: Option<ColorRef>,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(v: &bool) -> bool {
    !*v
}

fn is_square(v: &Corners) -> bool {
    *v == Corners::SQUARE
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_fit(v: &Fit) -> bool {
    *v == Fit::default()
}

impl Command {
    /// The command's wire name. Hashed by name rather than by discriminant,
    /// so reordering this enum can never silently rewrite a published digest.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Push { .. } => "push",
            Self::Pop => "pop",
            Self::Rect { .. } => "rect",
            Self::Ellipse { .. } => "ellipse",
            Self::Path { .. } => "path",
            Self::Sprite { .. } => "sprite",
        }
    }

    /// The asset this command draws, if it draws one.
    #[must_use]
    pub fn asset(&self) -> Option<&AssetRef> {
        match self {
            Self::Sprite { asset, .. } => Some(asset),
            _ => None,
        }
    }
}

/// Why a [`DrawList`] was refused.
///
/// Every variant names the bound or the rule and the amount by which the list
/// missed it. A refusal a caller cannot act on is a refusal that gets worked
/// around by lowering the bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawListError {
    /// More commands than [`MAX_DRAW_COMMANDS`].
    TooManyCommands {
        /// How many the list carried.
        found: usize,
        /// The bound it exceeded.
        limit: usize,
    },
    /// More path verbs, summed over every `Path`, than
    /// [`MAX_DRAW_PATH_VERBS`].
    TooManyPathVerbs {
        /// How many the list carried in total.
        found: usize,
        /// The bound it exceeded.
        limit: usize,
    },
    /// A `Pop` with no matching `Push`.
    UnmatchedPop {
        /// Index of the offending `Pop`.
        index: usize,
    },
    /// The list ended with pushes still on the stack.
    UnclosedPush {
        /// How many were left open.
        depth: usize,
    },
    /// A filled, closed `Path` whose control polygon is not convex.
    NonConvexFill {
        /// Index of the offending command.
        index: usize,
        /// Index within the path's control polygon where the turn reversed.
        at_point: usize,
    },
    /// A filled, closed `Path` with too few points to bound any area.
    DegenerateFill {
        /// Index of the offending command.
        index: usize,
        /// How many control-polygon points it carried.
        points: usize,
    },
    /// The list named a version this build does not implement.
    UnknownVersion {
        /// What the list said.
        found: String,
        /// What this build speaks.
        expected: &'static str,
    },
}

impl std::fmt::Display for DrawListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyCommands { found, limit } => write!(
                f,
                "a draw list carries {found} commands, {} over the {limit} a canvas may hold \
                 (MAX_DRAW_COMMANDS)",
                found - limit
            ),
            Self::TooManyPathVerbs { found, limit } => write!(
                f,
                "a draw list carries {found} path verbs, {} over the {limit} a canvas may hold \
                 (MAX_DRAW_PATH_VERBS)",
                found - limit
            ),
            Self::UnmatchedPop { index } => write!(
                f,
                "command {index} is a Pop with no matching Push; an unbalanced stack leaves a \
                 transform or a clip in force over commands that never asked for it"
            ),
            Self::UnclosedPush { depth } => write!(
                f,
                "the draw list ends with {depth} Push(es) still open; every Push needs its Pop"
            ),
            Self::NonConvexFill { index, at_point } => write!(
                f,
                "command {index} is a filled closed Path whose control polygon turns back on \
                 itself at point {at_point}; epaint fills convex polygons only, so this would \
                 draw wrong rather than slow. Decompose it into convex pieces"
            ),
            Self::DegenerateFill { index, points } => write!(
                f,
                "command {index} is a filled closed Path with {points} control point(s), which \
                 bounds no area; a fill needs at least three"
            ),
            Self::UnknownVersion { found, expected } => write!(
                f,
                "a draw list names version {found:?}; this build speaks {expected}. The command \
                 set is closed and versioned: an unknown command is refused with the whole list, \
                 never skipped"
            ),
        }
    }
}

impl std::error::Error for DrawListError {}

/// A canvas's picture: a flat command stream that passed every bound in §5.
///
/// The only constructor is [`DrawList::new`], and the only deserialization
/// path goes through it, so a `DrawList` that exists has been checked. That is
/// what "an over-budget list MUST NOT reach a frame" means structurally rather
/// than by discipline.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(into = "Wire")]
pub struct DrawList {
    commands: Vec<Command>,
    /// Summed over every `Path`, computed once at construction. Read by the
    /// digest's cost accounting and by tests; never recomputed, so it cannot
    /// disagree with what the bound was checked against.
    path_verbs: usize,
}

/// The wire shape of a [`DrawList`]: a version and a command vector.
///
/// A separate struct so `serde` cannot construct a `DrawList` without going
/// through [`DrawList::new`]. Deserializing straight into the real type would
/// make every bound in §5 a rule that holds for Rust callers and not for the
/// Lua ones the bounds exist for.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: String,
    commands: Vec<Command>,
}

impl From<DrawList> for Wire {
    fn from(list: DrawList) -> Self {
        Self {
            version: VERSION.to_owned(),
            commands: list.commands,
        }
    }
}

impl<'de> Deserialize<'de> for DrawList {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(de)?;
        if wire.version != VERSION {
            return Err(serde::de::Error::custom(DrawListError::UnknownVersion {
                found: wire.version,
                expected: VERSION,
            }));
        }
        Self::new(wire.commands).map_err(serde::de::Error::custom)
    }
}

impl DrawList {
    /// Check `commands` against every bound in §5 and take ownership of them.
    ///
    /// # Errors
    /// [`DrawListError`], naming which bound or rule and by how much. The
    /// checks run in a fixed order — counts, then stack balance, then
    /// convexity — so one malformed list always reports the same first
    /// problem and a caller fixing them one at a time makes progress.
    pub fn new(commands: Vec<Command>) -> Result<Self, DrawListError> {
        if commands.len() > MAX_DRAW_COMMANDS {
            return Err(DrawListError::TooManyCommands {
                found: commands.len(),
                limit: MAX_DRAW_COMMANDS,
            });
        }
        let path_verbs: usize = commands
            .iter()
            .map(|c| match c {
                Command::Path { verbs, .. } => verbs.len(),
                _ => 0,
            })
            .sum();
        if path_verbs > MAX_DRAW_PATH_VERBS {
            return Err(DrawListError::TooManyPathVerbs {
                found: path_verbs,
                limit: MAX_DRAW_PATH_VERBS,
            });
        }
        let mut depth = 0_usize;
        for (index, command) in commands.iter().enumerate() {
            match command {
                Command::Push { .. } => depth += 1,
                Command::Pop => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or(DrawListError::UnmatchedPop { index })?;
                }
                Command::Path {
                    verbs,
                    closed: true,
                    paint,
                } if paint.fill.is_some() => check_convex(index, verbs)?,
                _ => {}
            }
        }
        if depth != 0 {
            return Err(DrawListError::UnclosedPush { depth });
        }
        Ok(Self {
            commands,
            path_verbs,
        })
    }

    /// The commands, in order.
    #[must_use]
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Every theme token this list paints with, fills and strokes alike, in
    /// command order, duplicates kept.
    ///
    /// The one place a caller asks *what colours does this picture use*. A
    /// contrast check written against `Props.text` sees nothing at all once
    /// a mark moves from a text leaf to a canvas — which is exactly what
    /// happened to `component::list`'s unordered markers on 2026-09-05 — so
    /// the walk belongs next to the list rather than copied into every test
    /// module that needs it.
    ///
    /// A [`ColorRef::Rgba`] literal has no token name and is skipped;
    /// nothing in this library's shipped components paints one, and gate
    /// C1-8 is what keeps it that way.
    #[must_use]
    pub fn token_colours(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for command in &self.commands {
            let paint = match command {
                Command::Rect { paint, .. }
                | Command::Ellipse { paint, .. }
                | Command::Path { paint, .. } => paint,
                _ => continue,
            };
            for reference in paint
                .fill
                .iter()
                .chain(paint.stroke.as_ref().map(|s| &s.color))
            {
                if let ColorRef::Token(name) = reference {
                    out.push(name.as_str());
                }
            }
        }
        out
    }

    /// How many commands the list carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the list draws nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// How many path verbs the list carries, summed over every `Path`.
    #[must_use]
    pub fn path_verbs(&self) -> usize {
        self.path_verbs
    }

    /// Whether any command draws a registered asset.
    ///
    /// This is the **narrower** of `contracts/draw-list.md` §6's two
    /// predicates, and the whole of what
    /// [`crate::frame::PaintContent::is_hosted`] asks a canvas: a list with no
    /// `Sprite` in it is fully digest-visible — every coordinate it draws is
    /// hashed — so it is never hosted. A list with one is hosted, because the
    /// digest sees the asset's name and its rects and never its decoded
    /// pixels.
    ///
    /// It is emphatically **not** the question "can this repaint itself",
    /// which [`crate::frame::PaintContent::repaints_itself`] answers `true` to
    /// for every canvas regardless. The two diverge on exactly this case, and
    /// collapsing them is the ambient hole FR-030 exists to close.
    #[must_use]
    pub fn references_assets(&self) -> bool {
        self.commands.iter().any(|c| c.asset().is_some())
    }

    /// Every asset this list draws, in command order, with duplicates.
    pub fn assets(&self) -> impl Iterator<Item = &AssetRef> {
        self.commands.iter().filter_map(Command::asset)
    }

    /// Every theme token this list paints with — each fill and each stroke
    /// that is a [`ColorRef::Token`] — in command order, with duplicates.
    ///
    /// A literal [`ColorRef::Rgba`] is not a token and is skipped. This is
    /// how a contrast check reads a glyph's ink the way it reads a text
    /// node's `foreground` binding.
    pub fn color_tokens(&self) -> impl Iterator<Item = &str> {
        self.commands
            .iter()
            .filter_map(|command| match command {
                Command::Rect { paint, .. }
                | Command::Ellipse { paint, .. }
                | Command::Path { paint, .. } => Some(paint),
                Command::Push { .. } | Command::Pop | Command::Sprite { .. } => None,
            })
            .flat_map(|paint| {
                paint
                    .fill
                    .iter()
                    .chain(paint.stroke.as_ref().map(|stroke| &stroke.color))
            })
            .filter_map(|reference| match reference {
                ColorRef::Token(name) => Some(name.as_str()),
                ColorRef::Rgba(_) => None,
            })
    }
}

/// Refuse a filled closed path whose control polygon is not convex.
///
/// The test is the O(n) cross-product sign sweep `contracts/draw-list.md` §5
/// names, run over the *control* polygon rather than a flattening of the
/// curve. That is deliberate and it is conservative in the safe direction: a
/// bézier lies inside the convex hull of its own control points, so a convex,
/// one-way-turning control polygon bounds a convex region and is certainly
/// safe to fill. The converse does not hold — a convex curve can have a
/// control polygon that wiggles — so such a path is refused by name rather
/// than filled and hoped for. Refusing a fillable shape costs the author one
/// `MoveTo`; filling an unfillable one costs the reader a wrong picture under
/// a digest that says the frame is right.
///
/// Collinear points are allowed: a cross product of zero is a straight run,
/// not a reversal, and refusing it would reject every rectangle written as a
/// path.
fn check_convex(index: usize, verbs: &[PathVerb]) -> Result<(), DrawListError> {
    let points: Vec<Point> = verbs.iter().flat_map(|v| v.points()).collect();
    if points.len() < 3 {
        return Err(DrawListError::DegenerateFill {
            index,
            points: points.len(),
        });
    }
    let n = points.len();
    let mut sign = 0_i8;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        let c = points[(i + 2) % n];
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        // Exact zero rather than an epsilon: an epsilon here would be a second,
        // unstated tolerance sitting next to the digest's exact float rule, and
        // the two would disagree about the same list on two targets.
        let turn = if cross > 0.0 {
            1
        } else if cross < 0.0 {
            -1
        } else {
            0
        };
        if turn == 0 {
            continue;
        }
        if sign == 0 {
            sign = turn;
        } else if sign != turn {
            return Err(DrawListError::NonConvexFill {
                index,
                at_point: (i + 1) % n,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Affine, AssetRef, ColorRef, Command, Corners, DrawList, DrawListError, Fit,
        MAX_DRAW_COMMANDS, MAX_DRAW_PATH_VERBS, Paint, PathVerb, Stroke, VERSION, Width,
    };
    use crate::geom::{Point, Rect, Scale, Size};

    fn red() -> ColorRef {
        ColorRef::Rgba([255, 0, 0, 255])
    }

    fn box_command() -> Command {
        Command::Rect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radius: Corners::SQUARE,
            snap: false,
            paint: Paint::filled(red()),
        }
    }

    fn square_path(paint: Paint, closed: bool) -> Command {
        Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 10.0)),
                PathVerb::LineTo(Point::new(0.0, 10.0)),
            ],
            closed,
            paint,
        }
    }

    /// The classic non-convex outline: an arrowhead notch on one edge, so the
    /// polygon turns one way three times and the other way once.
    fn chevron(paint: Paint) -> Command {
        Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
                PathVerb::LineTo(Point::new(5.0, 5.0)),
                PathVerb::LineTo(Point::new(10.0, 10.0)),
                PathVerb::LineTo(Point::new(0.0, 10.0)),
            ],
            closed: true,
            paint,
        }
    }

    #[test]
    fn a_balanced_list_is_accepted_and_reports_its_own_costs() {
        let list = DrawList::new(vec![
            Command::Push {
                transform: Some(Affine::translate(4.0, 4.0)),
                clip: None,
                opacity: Some(0.5),
            },
            box_command(),
            square_path(Paint::filled(red()), true),
            Command::Pop,
        ])
        .unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!(list.path_verbs(), 4);
        assert!(!list.is_empty());
        assert!(!list.references_assets());
    }

    /// T123: the bound names itself and by how much the list missed it. A
    /// refusal that only says "too big" is one an author works around by
    /// deleting at random.
    #[test]
    fn draw_budget_refuses_too_many_commands_by_name_and_amount() {
        // The contract's own numbers, pinned. Every other assertion in this
        // test is written *relative* to the constant, so raising the constant
        // would keep them all green while quietly buying an author a hundred
        // times the budget `contracts/draw-list.md` §5 allows. Changing these
        // two lines is allowed; changing them silently is not.
        assert_eq!(MAX_DRAW_COMMANDS, 4096);
        assert_eq!(MAX_DRAW_PATH_VERBS, 16_384);

        let over = vec![box_command(); MAX_DRAW_COMMANDS + 7];
        let err = DrawList::new(over).unwrap_err();
        assert_eq!(
            err,
            DrawListError::TooManyCommands {
                found: MAX_DRAW_COMMANDS + 7,
                limit: MAX_DRAW_COMMANDS,
            }
        );
        let shown = err.to_string();
        assert!(shown.contains("MAX_DRAW_COMMANDS"), "{shown}");
        assert!(shown.contains("7 over"), "names the overage: {shown}");

        // The bound itself is legal: an off-by-one here would make the
        // constant mean something other than what it says.
        assert!(DrawList::new(vec![box_command(); MAX_DRAW_COMMANDS]).is_ok());
    }

    #[test]
    fn draw_budget_refuses_too_many_path_verbs_by_name_and_amount() {
        let verbs = vec![PathVerb::LineTo(Point::new(1.0, 1.0)); MAX_DRAW_PATH_VERBS / 2 + 3];
        let one = Command::Path {
            verbs: verbs.clone(),
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(1.0),
                color: red(),
            }),
        };
        let err = DrawList::new(vec![one.clone(), one]).unwrap_err();
        assert_eq!(
            err,
            DrawListError::TooManyPathVerbs {
                found: MAX_DRAW_PATH_VERBS + 6,
                limit: MAX_DRAW_PATH_VERBS,
            }
        );
        let shown = err.to_string();
        assert!(shown.contains("MAX_DRAW_PATH_VERBS"), "{shown}");
        assert!(shown.contains("6 over"), "names the overage: {shown}");
    }

    /// The verb bound sums across `Path`s rather than applying per command:
    /// two paths of half the bound each are over it, and a list that only
    /// checked one command at a time would accept them.
    #[test]
    fn draw_budget_sums_path_verbs_across_commands() {
        let half = vec![PathVerb::LineTo(Point::new(1.0, 1.0)); MAX_DRAW_PATH_VERBS / 2];
        let one = Command::Path {
            verbs: half,
            closed: false,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(1.0),
                color: red(),
            }),
        };
        assert!(DrawList::new(vec![one.clone()]).is_ok());
        assert!(DrawList::new(vec![one.clone(), one.clone()]).is_ok());
        let three = DrawList::new(vec![one.clone(), one.clone(), one]);
        assert!(matches!(three, Err(DrawListError::TooManyPathVerbs { .. })));
    }

    /// T124: a non-convex filled path is refused by name. Not degraded, not
    /// drawn as its convex hull, not silently stroked instead.
    #[test]
    fn a_non_convex_filled_path_is_refused_at_construction() {
        let err = DrawList::new(vec![chevron(Paint::filled(red()))]).unwrap_err();
        assert!(
            matches!(err, DrawListError::NonConvexFill { index: 0, .. }),
            "{err:?}"
        );
        let shown = err.to_string();
        assert!(shown.contains("convex"), "{shown}");
        assert!(shown.contains("Decompose"), "names the fix: {shown}");
    }

    /// The other three sides of the same claim: convexity is checked for
    /// *filled closed* paths and for nothing else, because those are the only
    /// ones `epaint` tessellates as a convex polygon.
    #[test]
    fn convexity_is_checked_only_where_epaint_needs_it() {
        let stroke = Paint::stroked(Stroke {
            width: Width::Logical(1.0),
            color: red(),
        });
        assert!(
            DrawList::new(vec![chevron(stroke.clone())]).is_ok(),
            "a stroked chevron is a polyline; epaint strokes any polyline"
        );
        let open = Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
                PathVerb::LineTo(Point::new(5.0, 5.0)),
                PathVerb::LineTo(Point::new(10.0, 10.0)),
            ],
            closed: false,
            paint: Paint::filled(red()),
        };
        assert!(
            DrawList::new(vec![open]).is_ok(),
            "an open path has no interior to fill convexly"
        );
        assert!(
            DrawList::new(vec![square_path(Paint::filled(red()), true)]).is_ok(),
            "a convex filled path is the case that must keep working"
        );
        assert!(
            DrawList::new(vec![square_path(stroke, false)]).is_ok(),
            "a stroked open path is unchecked"
        );
    }

    /// A rectangle written as a path has three collinear runs if the author
    /// puts a midpoint on an edge. Collinear is a straight run, not a
    /// reversal, and refusing it would reject a shape `epaint` fills
    /// perfectly well.
    #[test]
    fn collinear_points_do_not_read_as_a_reversal() {
        let with_midpoints = Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(5.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 10.0)),
                PathVerb::LineTo(Point::new(0.0, 10.0)),
            ],
            closed: true,
            paint: Paint::filled(red()),
        };
        assert!(DrawList::new(vec![with_midpoints]).is_ok());
    }

    /// A curved but convex path is accepted, which is what makes the control
    /// polygon the right thing to test: a lens shape's control points bound
    /// it, and they turn one way.
    #[test]
    fn a_convex_curved_path_is_accepted() {
        let lens = Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 5.0)),
                PathVerb::QuadTo {
                    ctrl: Point::new(5.0, 0.0),
                    to: Point::new(10.0, 5.0),
                },
                PathVerb::QuadTo {
                    ctrl: Point::new(5.0, 10.0),
                    to: Point::new(0.0, 5.0),
                },
            ],
            closed: true,
            paint: Paint::filled(red()),
        };
        assert!(DrawList::new(vec![lens]).is_ok());
    }

    #[test]
    fn a_filled_path_with_no_area_is_refused() {
        let line = Command::Path {
            verbs: vec![
                PathVerb::MoveTo(Point::new(0.0, 0.0)),
                PathVerb::LineTo(Point::new(10.0, 0.0)),
            ],
            closed: true,
            paint: Paint::filled(red()),
        };
        let err = DrawList::new(vec![line]).unwrap_err();
        assert_eq!(
            err,
            DrawListError::DegenerateFill {
                index: 0,
                points: 2
            }
        );
    }

    #[test]
    fn an_unbalanced_stack_is_refused_from_both_ends() {
        let push = Command::Push {
            transform: None,
            clip: Some(Rect::new(0.0, 0.0, 4.0, 4.0)),
            opacity: None,
        };
        assert_eq!(
            DrawList::new(vec![push.clone(), box_command()]).unwrap_err(),
            DrawListError::UnclosedPush { depth: 1 }
        );
        assert_eq!(
            DrawList::new(vec![box_command(), Command::Pop]).unwrap_err(),
            DrawListError::UnmatchedPop { index: 1 }
        );
        assert!(DrawList::new(vec![push, box_command(), Command::Pop]).is_ok());
    }

    /// T125: an unknown command discriminant refuses the **whole** list. The
    /// second assertion is the one that matters: the two legal commands beside
    /// it do not survive.
    #[test]
    fn drawlist_version_refuses_an_unknown_command_rather_than_skipping_it() {
        let json = r#"{"version":"drawlist-v1","commands":[
            "pop",
            {"blur":{"radius":4.0}},
            "pop"
        ]}"#;
        let err = serde_json::from_str::<DrawList>(json).unwrap_err();
        assert!(err.to_string().contains("blur"), "{err}");
        assert!(
            serde_json::from_str::<DrawList>(r#"{"version":"drawlist-v1","commands":["pop"]}"#)
                .is_err(),
            "the surviving commands are not salvaged either — this one is \
             refused for its unbalanced stack, so nothing about the bad list \
             could have been partially accepted"
        );
    }

    /// An unknown *field* on a known command is refused too: a `v2` list with
    /// a seventh field on `rect` is not a `v1` list with a field to ignore.
    #[test]
    fn drawlist_version_refuses_an_unknown_field_on_a_known_command() {
        let json = r#"{"version":"drawlist-v1","commands":[
            {"rect":{"rect":{"x":0,"y":0,"w":1,"h":1},"paint":{},"blend":"multiply"}}
        ]}"#;
        let err = serde_json::from_str::<DrawList>(json).unwrap_err();
        assert!(err.to_string().contains("blend"), "{err}");
    }

    #[test]
    fn drawlist_version_refuses_a_version_it_does_not_speak() {
        let json = r#"{"version":"drawlist-v2","commands":[]}"#;
        let err = serde_json::from_str::<DrawList>(json)
            .unwrap_err()
            .to_string();
        assert!(err.contains("drawlist-v2"), "{err}");
        assert!(err.contains(VERSION), "names what this build speaks: {err}");
    }

    /// Deserialization runs the same bounds construction does. Without this,
    /// every §5 bound would hold for a Rust caller and not for the Lua author
    /// the bounds were written for.
    #[test]
    fn drawlist_version_carries_the_construction_bounds_onto_the_wire() {
        let mut commands = String::from(r#"{"version":"drawlist-v1","commands":["#);
        for i in 0..=MAX_DRAW_COMMANDS {
            if i > 0 {
                commands.push(',');
            }
            commands.push_str(r#"{"push":{}},"pop""#);
        }
        commands.push_str("]}");
        let err = serde_json::from_str::<DrawList>(&commands).unwrap_err();
        assert!(err.to_string().contains("MAX_DRAW_COMMANDS"), "{err}");
    }

    #[test]
    fn a_list_round_trips_through_its_wire_form() {
        let list = DrawList::new(vec![
            Command::Push {
                transform: Some(Affine::translate(2.5, -1.5)),
                clip: Some(Rect::new(0.0, 0.0, 8.0, 8.0)),
                opacity: Some(0.25),
            },
            Command::Ellipse {
                center: Point::new(4.0, 4.0),
                radii: Size::new(2.0, 3.0),
                paint: Paint::filled(ColorRef::Token("status.ok".into())),
            },
            Command::Sprite {
                asset: AssetRef::host("logo.png"),
                dst: Rect::new(1.0, 1.0, 6.0, 6.0),
                src: None,
                fit: Fit::Contain,
                tint: Some(red()),
            },
            Command::Pop,
        ])
        .unwrap();
        let json = serde_json::to_string(&list).unwrap();
        assert!(json.contains(r#""version":"drawlist-v1""#), "{json}");
        let back: DrawList = serde_json::from_str(&json).unwrap();
        assert_eq!(back, list);
        assert!(back.references_assets());
        assert_eq!(back.assets().count(), 1);
        assert_eq!(back.assets().next().unwrap().wire(), "host/logo.png");
    }

    /// §6's narrow predicate. A geometry-only list is digest-visible, so it
    /// never answers `true` here however much it draws.
    #[test]
    fn a_geometry_only_list_references_no_assets_however_large() {
        let list = DrawList::new(vec![box_command(); 512]).unwrap();
        assert!(!list.references_assets());
        assert_eq!(list.assets().count(), 0);
    }

    /// `Width::Device` resolves at execution time, so the same list draws a
    /// one-device-pixel hairline at every scale — and the list's own bytes,
    /// and therefore its digest, do not move when the scale does.
    #[test]
    fn a_device_width_resolves_against_the_frames_scale() {
        let one_pixel = Width::Device(1.0);
        assert_eq!(one_pixel.logical(Scale::ONE), 1.0);
        assert_eq!(one_pixel.logical(Scale::new(2.0).unwrap()), 0.5);
        assert_eq!(Width::Logical(1.0).logical(Scale::new(2.0).unwrap()), 1.0);
    }

    /// Composition is scale-then-translate, and it is associative for this
    /// family — which is the property that makes a flat `Push`/`Pop` stack a
    /// complete replacement for nested groups (§2).
    #[test]
    fn transforms_compose_in_one_order_and_associate() {
        let a = Affine {
            tx: 3.0,
            ty: 5.0,
            sx: 2.0,
            sy: 2.0,
        };
        let b = Affine::translate(1.0, 1.0);
        let c = Affine {
            tx: 0.0,
            ty: 0.0,
            sx: 0.5,
            sy: 4.0,
        };
        let left = c.then(b.then(a));
        let right = c.then(b).then(a);
        assert_eq!(left, right);
        // Hand-computed rather than derived from `then`: b at scale 2 moves
        // twice as far.
        assert_eq!(
            b.then(a),
            Affine {
                tx: 5.0,
                ty: 7.0,
                sx: 2.0,
                sy: 2.0,
            }
        );
        assert_eq!(
            Affine::IDENTITY.apply(Point::new(4.0, 5.0)),
            Point::new(4.0, 5.0)
        );
        assert_eq!(
            a.apply_rect(Rect::new(1.0, 1.0, 2.0, 2.0)),
            Rect::new(5.0, 7.0, 4.0, 4.0)
        );
    }

    #[test]
    fn every_command_and_verb_names_itself() {
        assert_eq!(box_command().as_str(), "rect");
        assert_eq!(Command::Pop.as_str(), "pop");
        assert_eq!(
            Command::Push {
                transform: None,
                clip: None,
                opacity: None
            }
            .as_str(),
            "push"
        );
        assert_eq!(PathVerb::Close.as_str(), "close");
        assert_eq!(PathVerb::MoveTo(Point::ZERO).as_str(), "move-to");
        assert_eq!(Fit::Contain.as_str(), "contain");
        assert_eq!(Corners::all(3.0).top_left, 3.0);
    }
}
