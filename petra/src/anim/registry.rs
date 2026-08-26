//! Transition definitions and the registry that resolves a name to one.
//!
//! `crate::tree::TransitionRef`'s own doc states the arrangement: *"The tree
//! holds a name, not a definition ... the registry that resolves the name
//! lives in [`crate::anim`]. An unresolvable name is a tree-acceptance
//! error."* This module is that registry, and
//! [`TransitionRegistry::declare_into`] is what makes the second sentence true
//! by construction rather than by discipline — the set of names tree
//! acceptance will allow is *copied from* the set the engine can resolve, so
//! the two cannot drift apart in one direction without the other noticing.

use std::collections::BTreeMap;
use std::fmt;

use super::curve::{Curve, KeyframeTrack};
use super::spring::Spring;
use super::value::{AnimVector, PropertyKind, Thresholds};

/// What drives one property: a spring, a timed curve, or a keyframe track.
///
/// `contracts/animation.md`: *"a keyframe track and a spring never drive the
/// same property simultaneously (last declaration wins, recorded)"*. Making
/// this an enum keyed by property in [`TransitionDef`] is what makes
/// "simultaneously" unrepresentable: there is one slot per property, so the
/// second declaration replaces the first, and [`TransitionDef::overrides`]
/// is where the replacement is recorded rather than lost.
#[derive(Clone, Debug, PartialEq)]
pub enum Timing {
    /// A damped spring, retargeted by trajectory evaluation.
    Spring(Spring),
    /// A duration and an easing.
    Curve(Curve),
    /// An absolute per-property value track.
    ///
    /// A track defines its own values, so while it runs it *replaces* the
    /// laid-out value rather than interpolating toward it; when the track
    /// ends the property snaps to the layout target exactly. That keeps a
    /// keyframe flourish from leaving residue in the settled digest, which is
    /// the rule the whole settle section turns on.
    Keyframes(KeyframeTrack),
}

impl Timing {
    /// A short name for a message.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Spring(_) => "spring",
            Self::Curve(_) => "curve",
            Self::Keyframes(_) => "keyframes",
        }
    }

    /// The trajectory-local time an **ambient** transition samples at:
    /// `t mod duration` for a keyframe track, `t` unchanged for anything else.
    ///
    /// This one expression is the whole of `contracts/draw-list.md` §8's
    /// widening. Before it, an ambient keyframe track ran once, hit
    /// [`KeyframeTrack::duration`], and snapped to the layout target — so
    /// "ambient" meant a host had to notice the end and re-trigger it, every
    /// host, forever, and a host that forgot got a surface that animated once
    /// and then sat still while the settle contract still excluded it. Looping
    /// in the engine makes the declaration mean what it says.
    ///
    /// The modulo is what keeps the track from ever reaching the `t >=
    /// duration` branch in [`Timing::sample`], so an ambient track also never
    /// reports `settled` — which is the definition of ambient, arrived at
    /// rather than special-cased.
    ///
    /// A spring or a curve returns `t` untouched: neither can loop, and
    /// [`TransitionBuilder::build`] already refuses an ambient definition that
    /// drives either of them. A zero-duration track returns `t` untouched too,
    /// because a modulo by zero has no answer; such a track cannot be built —
    /// [`KeyframeTrack::new`] refuses one — and this is what the function does
    /// rather than divide anyway.
    #[must_use]
    pub fn looped_time(&self, t: f64) -> f64 {
        match self {
            Self::Keyframes(track) => {
                let duration = track.duration();
                if duration > 0.0 { t % duration } else { t }
            }
            Self::Spring(_) | Self::Curve(_) => t,
        }
    }

    /// Value, velocity, and whether the trajectory has settled, at `t`
    /// seconds into a trajectory that began at `origin` moving at
    /// `velocity0`, heading for `target`.
    ///
    /// # Panics
    /// If the vectors disagree in length, or `t` is negative.
    #[must_use]
    pub fn sample(
        &self,
        origin: AnimVector,
        velocity0: AnimVector,
        target: AnimVector,
        t: f64,
        thresholds: Thresholds,
    ) -> Sample {
        match self {
            Self::Spring(spring) => {
                let (value, velocity) = spring.evaluate(origin, velocity0, target, t);
                let settled = thresholds.settled(value, target, velocity);
                Sample {
                    value,
                    velocity,
                    settled,
                }
            }
            Self::Curve(curve) => {
                let (value, velocity) = curve.evaluate(origin, target, t);
                Sample {
                    value,
                    velocity,
                    settled: t >= curve.duration(),
                }
            }
            Self::Keyframes(track) => {
                if t >= track.duration() {
                    return Sample {
                        value: target,
                        velocity: AnimVector::zeros(target.len()),
                        settled: true,
                    };
                }
                let (value, velocity) = track.evaluate(t);
                Sample {
                    value,
                    velocity,
                    settled: false,
                }
            }
        }
    }
}

/// One evaluation of a [`Timing`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Where the property is now.
    pub value: AnimVector,
    /// How fast it is moving, per second. This is what a retarget carries.
    pub velocity: AnimVector,
    /// Whether it is at the target and at rest.
    pub settled: bool,
}

/// How a node whose transition is still running behaves when it is removed
/// from the tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ExitRule {
    /// The node leaves on the frame it is removed. The default: a node the
    /// application no longer describes is gone unless something says
    /// otherwise.
    #[default]
    CompleteInstantly,
    /// The node stays placed and runs one more trajectory, then leaves.
    ///
    /// The exiting node keeps its last placement, its last paint payload and
    /// its last offered slot, and is re-inserted into the frame under the
    /// parent it had. If that parent is *also* gone, there is nowhere to
    /// re-insert it and the whole group completes instantly instead — a
    /// deliberate, stated degradation rather than a placement dangling with
    /// no parent, which would break the frame's Merkle root.
    RunExitTrack(Track),
}

/// One property's endpoint for an enter or exit flourish.
///
/// An exit track names where the property goes; an enter track names where it
/// comes from. Both are absolute values in the property's own units, which is
/// why an opacity fade is `Track { property: Opacity, value: 0.0 }` rather
/// than a multiplier nobody could read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    /// Which property the flourish drives.
    pub property: PropertyKind,
    /// The value at the far end: the exit's destination, or the enter's
    /// start.
    pub value: AnimVector,
}

impl Track {
    /// A fade to or from fully transparent.
    #[must_use]
    pub fn fade() -> Self {
        Self {
            property: PropertyKind::Opacity,
            value: AnimVector::scalar(0.0),
        }
    }

    /// A flourish on `property` reaching `value`.
    ///
    /// # Errors
    /// If `value`'s width does not match the property's component count.
    pub fn new(property: PropertyKind, value: AnimVector) -> Result<Self, String> {
        if value.len() != property.components() {
            return Err(format!(
                "{property:?} interpolates as {} component(s), but the track's value has {}",
                property.components(),
                value.len()
            ));
        }
        Ok(Self { property, value })
    }
}

/// A record that one declaration replaced another for the same property.
///
/// Kept rather than dropped because *"last declaration wins, recorded"* is
/// the contract's own wording, and a conflict silently resolved is a design
/// mistake nobody can find later.
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    /// The property both declarations named.
    pub property: PropertyKind,
    /// The timing kind that was displaced.
    pub replaced: &'static str,
    /// The timing kind that won.
    pub winner: &'static str,
}

impl fmt::Display for Override {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} was declared twice: the {} declaration replaced the {} one \
             (last declaration wins)",
            self.property, self.winner, self.replaced
        )
    }
}

/// A named transition: what it drives, how, and what happens at the edges of a
/// node's life.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionDef {
    timings: BTreeMap<PropertyKind, Timing>,
    overrides: Vec<Override>,
    enter: Option<Track>,
    exit: ExitRule,
    ambient: bool,
}

impl TransitionDef {
    /// Start building a definition.
    #[must_use]
    pub fn builder() -> TransitionBuilder {
        TransitionBuilder {
            timings: BTreeMap::new(),
            overrides: Vec::new(),
            enter: None,
            exit: ExitRule::CompleteInstantly,
            ambient: false,
            errors: Vec::new(),
        }
    }

    /// The timing driving `property`, if this definition drives it at all.
    #[must_use]
    pub fn timing(&self, property: PropertyKind) -> Option<&Timing> {
        self.timings.get(&property)
    }

    /// Every property this definition drives, in a stable order.
    pub fn properties(&self) -> impl Iterator<Item = PropertyKind> + '_ {
        self.timings.keys().copied()
    }

    /// Declarations that were replaced by a later one for the same property.
    #[must_use]
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }

    /// Where a newly created node's property starts from, if this definition
    /// declares an enter flourish.
    #[must_use]
    pub fn enter(&self) -> Option<Track> {
        self.enter
    }

    /// What a removed node does.
    #[must_use]
    pub fn exit(&self) -> &ExitRule {
        &self.exit
    }

    /// Whether this transition is a declared-endless animation.
    ///
    /// An ambient definition never settles and never blocks a driver's settle
    /// wait (`contracts/animation.md`, and
    /// [`crate::frame::TransitionActivity::is_settled`], which excludes
    /// `ambient` deliberately).
    #[must_use]
    pub fn is_ambient(&self) -> bool {
        self.ambient
    }
}

/// Builds a [`TransitionDef`], collecting every problem instead of stopping at
/// the first.
///
/// Collecting rather than short-circuiting for the same reason
/// `crate::tree::validate` reports the whole tree's violations: a host wiring
/// up ten transitions wants all ten messages on the first run, not one per
/// rebuild.
#[derive(Clone, Debug)]
pub struct TransitionBuilder {
    timings: BTreeMap<PropertyKind, Timing>,
    overrides: Vec<Override>,
    enter: Option<Track>,
    exit: ExitRule,
    ambient: bool,
    errors: Vec<String>,
}

impl TransitionBuilder {
    /// Drive `property` with `timing`.
    ///
    /// A second call for the same property replaces the first and records an
    /// [`Override`].
    #[must_use]
    pub fn drive(mut self, property: PropertyKind, timing: Timing) -> Self {
        if let Timing::Keyframes(track) = &timing
            && track.width() != property.components()
        {
            self.errors.push(format!(
                "a keyframe track driving {property:?} must have {} component(s), it has {}",
                property.components(),
                track.width()
            ));
            return self;
        }
        if let Some(previous) = self.timings.insert(property, timing.clone()) {
            self.overrides.push(Override {
                property,
                replaced: previous.kind_name(),
                winner: timing.kind_name(),
            });
        }
        self
    }

    /// Where a newly created node starts from.
    #[must_use]
    pub fn enter(mut self, track: Track) -> Self {
        self.enter = Some(track);
        self
    }

    /// What a removed node does.
    #[must_use]
    pub fn exit(mut self, rule: ExitRule) -> Self {
        self.exit = rule;
        self
    }

    /// Declare this transition endless.
    #[must_use]
    pub fn ambient(mut self, ambient: bool) -> Self {
        self.ambient = ambient;
        self
    }

    /// Finish, or report every problem found.
    ///
    /// # Errors
    /// A definition driving nothing, a flourish on a property the definition
    /// does not drive, an ambient definition that cannot in fact run forever,
    /// or any width mismatch collected along the way — joined into one
    /// message, so a caller sees all of them.
    pub fn build(mut self) -> Result<TransitionDef, String> {
        if self.timings.is_empty() {
            self.errors
                .push("a transition definition must drive at least one property".into());
        }
        if let Some(track) = self.enter
            && !self.timings.contains_key(&track.property)
        {
            self.errors.push(format!(
                "the enter track drives {:?}, which this definition does not animate",
                track.property
            ));
        }
        if let ExitRule::RunExitTrack(track) = &self.exit {
            if !self.timings.contains_key(&track.property) {
                self.errors.push(format!(
                    "the exit track drives {:?}, which this definition does not animate",
                    track.property
                ));
            }
            // An exiting node is spliced back into the frame with its whole
            // recorded subtree, and the root's animated value has to reach the
            // descendants or a child outlives its parent on screen. Opacity
            // propagates as a ratio and position as a translation; there is no
            // correct way to propagate a *size* change without re-negotiating
            // the subtree, and re-negotiating is exactly what an exiting node
            // cannot do, because it is no longer in the tree. So it is refused
            // here rather than silently applied to the root alone.
            if !matches!(
                track.property,
                PropertyKind::Opacity | PropertyKind::Position
            ) {
                self.errors.push(format!(
                    "an exit track drives Opacity or Position, not {:?} — an exiting \
                     subtree cannot be re-negotiated, so a size change has no correct \
                     propagation to its children",
                    track.property
                ));
            }
        }
        // An ambient transition that settles is a contradiction: it would be
        // excluded from the settle wait *and* stop moving, so a driver would
        // wait on nothing while the surface sat still. Only a spring with no
        // damping keeps going, and that is not what an author means here —
        // ambient motion is a keyframe loop, which `Timing::looped_time`
        // makes the engine run forever without a host re-triggering anything.
        // Refusing this is what stops "ambient" from becoming a way to opt out
        // of the settle contract by accident.
        if self.ambient {
            for (property, timing) in &self.timings {
                if !matches!(timing, Timing::Keyframes(_)) {
                    self.errors.push(format!(
                        "an ambient transition drives {property:?} with a {}, which settles; \
                         ambient motion must be a keyframe loop, which the engine runs on \
                         t mod duration",
                        timing.kind_name()
                    ));
                }
            }
        }
        if self.errors.is_empty() {
            Ok(TransitionDef {
                timings: self.timings,
                overrides: self.overrides,
                enter: self.enter,
                exit: self.exit,
                ambient: self.ambient,
            })
        } else {
            Err(self.errors.join("; "))
        }
    }
}

/// The set of transition definitions a host has registered.
#[derive(Clone, Debug, Default)]
pub struct TransitionRegistry {
    defs: BTreeMap<String, TransitionDef>,
}

impl TransitionRegistry {
    /// An empty registry. A tree naming any transition against it is refused.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `def` under `name`, answering the definition it replaced.
    ///
    /// Replacing is allowed and reported rather than refused: a host that
    /// re-registers a name during development wants the new definition, and a
    /// caller that did not mean to can see it happened.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        def: TransitionDef,
    ) -> Option<TransitionDef> {
        self.defs.insert(name.into(), def)
    }

    /// The definition `name` resolves to.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&TransitionDef> {
        self.defs.get(name)
    }

    /// Whether `name` resolves.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.defs.contains_key(name)
    }

    /// Every registered name, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.defs.keys().map(String::as_str).collect()
    }

    /// How many definitions are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Copy every registered name into the tree registry that accepts trees.
    ///
    /// This is the seam that makes `TransitionRef`'s documented promise real:
    /// tree acceptance refuses a name it does not hold
    /// (`crate::tree::validate`, `Violation::UnregisteredTransition`), and the
    /// only names it holds are the ones the engine can actually resolve,
    /// because they came from here. A host that registers a definition and
    /// forgets to declare it would otherwise get a tree refused for a
    /// transition that exists; a host that declares a name with no definition
    /// would get a node that silently never animates. Copying in one
    /// direction, from the definitions, closes both.
    pub fn declare_into(&self, registry: &mut crate::tree::Registry) {
        for name in self.defs.keys() {
            registry.register_transition(name.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExitRule, Timing, Track, TransitionDef, TransitionRegistry};
    use crate::anim::curve::{CubicBezier, Curve, Keyframe, KeyframeTrack};
    use crate::anim::spring::Spring;
    use crate::anim::value::{AnimVector, PropertyKind};
    use crate::tree::{NodeKind, Registry, ViewNode, validate};

    fn spring_move() -> TransitionDef {
        TransitionDef::builder()
            .drive(PropertyKind::Position, Timing::Spring(Spring::default()))
            .build()
            .unwrap()
    }

    #[test]
    fn a_definition_must_drive_something() {
        let err = TransitionDef::builder().build().unwrap_err();
        assert!(err.contains("at least one property"), "{err}");
    }

    /// The contract's conflict rule: last declaration wins, and the loss is
    /// recorded rather than silent.
    #[test]
    fn a_second_declaration_wins_and_is_recorded() {
        let track = KeyframeTrack::new(vec![
            Keyframe {
                time: 0.0,
                value: AnimVector::zeros(2),
                easing_in: CubicBezier::LINEAR,
            },
            Keyframe {
                time: 0.5,
                value: AnimVector::new([10.0, 10.0, 0.0, 0.0], 2),
                easing_in: CubicBezier::LINEAR,
            },
        ])
        .unwrap();
        let def = TransitionDef::builder()
            .drive(PropertyKind::Position, Timing::Spring(Spring::default()))
            .drive(PropertyKind::Position, Timing::Keyframes(track))
            .build()
            .unwrap();
        assert_eq!(def.overrides().len(), 1);
        assert_eq!(def.overrides()[0].replaced, "spring");
        assert_eq!(def.overrides()[0].winner, "keyframes");
        assert!(matches!(
            def.timing(PropertyKind::Position),
            Some(Timing::Keyframes(_))
        ));
        let shown = def.overrides()[0].to_string();
        assert!(shown.contains("last declaration wins"), "{shown}");
    }

    #[test]
    fn a_flourish_on_an_undriven_property_is_refused() {
        let err = TransitionDef::builder()
            .drive(PropertyKind::Position, Timing::Spring(Spring::default()))
            .exit(ExitRule::RunExitTrack(Track::fade()))
            .build()
            .unwrap_err();
        assert!(err.contains("does not animate"), "{err}");
    }

    #[test]
    fn a_keyframe_track_of_the_wrong_width_is_refused() {
        let scalar_track = KeyframeTrack::new(vec![
            Keyframe {
                time: 0.0,
                value: AnimVector::scalar(0.0),
                easing_in: CubicBezier::LINEAR,
            },
            Keyframe {
                time: 1.0,
                value: AnimVector::scalar(1.0),
                easing_in: CubicBezier::LINEAR,
            },
        ])
        .unwrap();
        let err = TransitionDef::builder()
            .drive(PropertyKind::Position, Timing::Keyframes(scalar_track))
            .build()
            .unwrap_err();
        assert!(err.contains("2 component(s)"), "{err}");
    }

    /// Ambient cannot be a way to opt a settling transition out of the settle
    /// contract.
    #[test]
    fn an_ambient_definition_that_would_settle_is_refused() {
        let err = TransitionDef::builder()
            .drive(
                PropertyKind::Opacity,
                Timing::Curve(Curve::new(0.3, CubicBezier::LINEAR).unwrap()),
            )
            .ambient(true)
            .build()
            .unwrap_err();
        assert!(err.contains("must be a keyframe loop"), "{err}");
        assert!(err.contains("t mod duration"), "{err}");
    }

    /// A track that loops, for the two tests below.
    fn looping_track() -> KeyframeTrack {
        KeyframeTrack::new(vec![
            Keyframe {
                time: 0.0,
                value: AnimVector::scalar(0.0),
                easing_in: CubicBezier::LINEAR,
            },
            Keyframe {
                time: 2.0,
                value: AnimVector::scalar(1.0),
                easing_in: CubicBezier::LINEAR,
            },
        ])
        .unwrap()
    }

    /// `contracts/draw-list.md` §8's widening: the **engine** loops an ambient
    /// keyframe track on `t mod duration`, rather than every host being
    /// obliged to notice the end and re-trigger it.
    ///
    /// Hand-computed wrap points rather than recomputed from `duration()`: a
    /// test that spelled `t % track.duration()` would agree with a sign error
    /// as readily as with the truth.
    #[test]
    fn an_ambient_keyframe_track_loops_on_t_mod_duration() {
        let timing = Timing::Keyframes(looping_track());
        assert_eq!(timing.looped_time(0.0), 0.0);
        assert_eq!(timing.looped_time(0.5), 0.5);
        assert_eq!(timing.looped_time(1.75), 1.75);
        assert_eq!(timing.looped_time(2.0), 0.0, "the wrap point");
        assert_eq!(timing.looped_time(2.5), 0.5);
        assert_eq!(timing.looped_time(7.25), 1.25, "many laps later");

        // A spring and a curve pass through: neither can loop, and an ambient
        // definition that drove one is refused at build time anyway.
        let spring = Timing::Spring(Spring::default());
        assert_eq!(spring.looped_time(7.25), 7.25);
        let curve = Timing::Curve(Curve::new(0.3, CubicBezier::LINEAR).unwrap());
        assert_eq!(curve.looped_time(7.25), 7.25);
    }

    /// The consequence that matters: a looped track never settles, so a
    /// declared-ambient surface keeps moving instead of snapping to its layout
    /// target one duration in.
    ///
    /// The second half is the control. The *same* track sampled at the *same*
    /// unwrapped clock does settle — so this test is about the loop and not
    /// about the track being unable to settle at all.
    #[test]
    fn a_looped_track_never_settles_where_the_same_track_unlooped_does() {
        let timing = Timing::Keyframes(looping_track());
        let origin = AnimVector::scalar(0.0);
        let target = AnimVector::scalar(1.0);
        let thresholds = PropertyKind::Opacity.thresholds();
        for lap in 0..5 {
            let t = 2.5 + f64::from(lap) * 2.0;
            let looped = timing.sample(
                origin,
                AnimVector::scalar(0.0),
                target,
                timing.looped_time(t),
                thresholds,
            );
            assert!(
                !looped.settled,
                "lap {lap}: an ambient track must not settle"
            );
            let straight = timing.sample(origin, AnimVector::scalar(0.0), target, t, thresholds);
            assert!(
                straight.settled,
                "lap {lap}: the same track without the loop does settle, which is                  what makes the assertion above about the loop"
            );
        }
    }

    /// The other half of §8, and the reason it lives here beside the loop
    /// rather than in `crate::draw`: **the engine owns time, so the draw list
    /// must not.**
    ///
    /// A list whose content is a function of wall-clock time, while the digest
    /// sees only the unevaluated expression, makes two identical digests draw
    /// two different pictures — the same exclusion `frame-identity.md`'s "Not
    /// covered" makes for wall-clock time, applied at construction. The rule
    /// is structural (`DrawList::new` takes commands and nothing else), and
    /// structure is exactly what a future field can quietly break, so this
    /// reads the module's own source and refuses a time-derived identifier
    /// appearing anywhere in its code.
    ///
    /// Comments are stripped first: the module doc has to be able to *say*
    /// "clock" and "frame counter" to explain the rule it is under.
    #[test]
    fn the_draw_list_takes_no_time_input() {
        /// Whole-word, case-insensitive, over code only.
        fn time_words(source: &str) -> Vec<String> {
            const FORBIDDEN: [&str; 11] = [
                "instant",
                "systemtime",
                "duration",
                "now",
                "elapsed",
                "clock",
                "time",
                "timestamp",
                "tick",
                "frame_index",
                "frame_count",
            ];
            let code: String = source
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut found = Vec::new();
            for token in code.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                let lower = token.to_ascii_lowercase();
                if FORBIDDEN.contains(&lower.as_str()) {
                    found.push(lower);
                }
            }
            found
        }

        // The sabotage half, first: a planted field must be caught, or the
        // clean result below proves nothing.
        let planted = "pub struct Command {\n    /// harmless\n    pub elapsed: f32,\n}";
        assert_eq!(time_words(planted), ["elapsed"]);
        let commented_only = "//! a clock, a frame counter, a timestamp\npub struct Command;";
        assert!(
            time_words(commented_only).is_empty(),
            "prose about the rule is not a violation of it"
        );

        let found = time_words(include_str!("../draw/mod.rs"));
        assert!(
            found.is_empty(),
            "crate::draw names {found:?} in its code; a draw list may not take a \
             time-derived value as a command field (contracts/draw-list.md §8). \
             A canvas whose content changes every frame rebuilds and resubmits \
             a whole new list and pays one digest change per frame."
        );
    }

    #[test]
    fn an_exit_track_on_size_is_refused() {
        let err = TransitionDef::builder()
            .drive(PropertyKind::Size, Timing::Spring(Spring::default()))
            .exit(ExitRule::RunExitTrack(
                Track::new(PropertyKind::Size, AnimVector::zeros(2)).unwrap(),
            ))
            .build()
            .unwrap_err();
        assert!(err.contains("Opacity or Position"), "{err}");
    }

    #[test]
    fn a_track_value_must_match_its_property_width() {
        let err = Track::new(PropertyKind::Position, AnimVector::scalar(0.0)).unwrap_err();
        assert!(err.contains("2 component(s)"), "{err}");
        assert!(Track::new(PropertyKind::Opacity, AnimVector::scalar(0.0)).is_ok());
    }

    /// The seam `TransitionRef`'s doc promises: a name the engine can resolve
    /// is a name tree acceptance allows, and one it cannot is refused.
    #[test]
    fn declaring_into_the_tree_registry_makes_exactly_the_resolvable_names_legal() {
        let mut anim = TransitionRegistry::new();
        anim.register("slide", spring_move());
        let mut tree_registry = Registry::new();
        anim.declare_into(&mut tree_registry);

        let good = ViewNode::new(NodeKind::Text, "t").with_transition("slide");
        assert!(validate(&good, &tree_registry).is_ok());

        let bad = ViewNode::new(NodeKind::Text, "t").with_transition("slyde");
        let errors = validate(&bad, &tree_registry).unwrap_err().to_string();
        assert!(errors.contains("slyde"), "{errors}");
        assert!(
            errors.contains("slide"),
            "the message names the legal set: {errors}"
        );
    }

    #[test]
    fn re_registering_a_name_answers_what_it_replaced() {
        let mut registry = TransitionRegistry::new();
        assert!(registry.register("a", spring_move()).is_none());
        assert!(registry.register("a", spring_move()).is_some());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.names(), ["a"]);
        assert!(TransitionRegistry::new().is_empty());
    }
}
