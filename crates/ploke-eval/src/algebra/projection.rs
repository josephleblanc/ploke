//! Projection, provenance, and read-surface vocabulary.

/// Deterministic projection from `Source` into an image.
///
/// `Image` is the value visible after projection. `Kernel` describes which
/// distinctions in `Source` the projection collapses or intentionally ignores.
/// A projection with a non-trivial kernel may still be useful, but callers
/// should not treat its image as source-identifying unless it is paired with a
/// sufficient witness.
pub trait Projection<Source> {
    type Image: Eq;
    type Kernel;
    type Error;

    fn project(&self, source: &Source) -> Result<Self::Image, Self::Error>;

    fn kernel(&self) -> Self::Kernel;

    fn fiber(&self, source: &Source) -> Result<Fiber<&Self, Self::Image>, Self::Error>
    where
        Self: Sized,
    {
        Fiber::of(self, source)
    }
}

impl<P, Source> Projection<Source> for &P
where
    P: Projection<Source>,
{
    type Image = P::Image;
    type Kernel = P::Kernel;
    type Error = P::Error;

    fn project(&self, source: &Source) -> Result<Self::Image, Self::Error> {
        (*self).project(source)
    }

    fn kernel(&self) -> Self::Kernel {
        (*self).kernel()
    }
}

/// A value that carries the result of a projection over `Source`.
///
/// This trait is intentionally independent from [`Witnessed`]. Some projected
/// values are not externally witnessed, and some witnessed values are not
/// projections.
pub trait Projected<Source> {
    type Projection: Projection<Source>;
    type Image: Eq;
    type Kernel;

    fn projection(&self) -> &Self::Projection;

    fn image(&self) -> &Self::Image;

    fn kernel(&self) -> &Self::Kernel;

    fn fiber(&self) -> Fiber<&Self::Projection, Self::Image>
    where
        Self::Image: Clone,
    {
        Fiber::new(self.projection(), self.image().clone())
    }
}

/// Bounded read-side access to a source.
///
/// This is the general read-surface counterpart to the intervention-specific
/// `Surface<C>` trait. A surface controls which targets are addressable and
/// what projection is produced from them.
pub trait ReadSurface<Source> {
    type Target;
    type View;
    type Error;

    fn read(&self, source: &Source, target: &Self::Target) -> Result<Self::View, Self::Error>;
}

/// One fiber descriptor of a projection: all sources that project to `image`.
///
/// The fiber does not itself contain every member. It stores the projection and
/// image that define membership:
///
/// `source` is in this fiber iff `projection.project(source) == image`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fiber<P, Image> {
    projection: P,
    image: Image,
}

impl<P, Image> Fiber<P, Image> {
    pub fn new(projection: P, image: Image) -> Self {
        Self { projection, image }
    }

    pub fn projection(&self) -> &P {
        &self.projection
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn into_parts(self) -> (P, Image) {
        (self.projection, self.image)
    }
}

impl<P, Image> Fiber<P, Image>
where
    Image: Eq,
{
    pub fn of<Source>(projection: P, source: &Source) -> Result<Self, P::Error>
    where
        P: Projection<Source, Image = Image>,
    {
        let image = projection.project(source)?;
        Ok(Self::new(projection, image))
    }

    pub fn contains<Source>(&self, source: &Source) -> Result<bool, P::Error>
    where
        P: Projection<Source, Image = Image>,
    {
        self.projection
            .project(source)
            .map(|image| image == self.image)
    }
}

/// Cross-runtime grounding provenance for a claim.
///
/// Provenance is not the projected payload. It is the grounding object that
/// names the producer and subject of a claim. Capability traits such as
/// [`VerifiableWith`] and [`ResolveWith`] describe what a given provenance
/// value can do against a concrete resolver/store/tree.
pub trait Provenance {
    type Producer;
    type Subject;

    fn producer(&self) -> &Self::Producer;

    fn subject(&self) -> &Self::Subject;
}

/// A value that carries provenance.
pub trait Witnessed {
    type Provenance: Provenance;

    fn provenance(&self) -> &Self::Provenance;
}

/// Provenance capability: check the provenance against an external resolver.
pub trait VerifiableWith<Resolver>: Provenance {
    type Error;

    fn verify_with(&self, resolver: &Resolver) -> Result<(), Self::Error>;
}

/// Provenance capability: recover the source representative named by the
/// provenance.
pub trait ResolveWith<Resolver>: VerifiableWith<Resolver> {
    type Source;

    fn resolve_with(&self, resolver: &Resolver) -> Result<Self::Source, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{Fiber, Projected, Projection, Provenance, ResolveWith, VerifiableWith, Witnessed};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Modulo(u32);

    impl Projection<u32> for Modulo {
        type Image = u32;
        type Kernel = &'static str;
        type Error = std::convert::Infallible;

        fn project(&self, source: &u32) -> Result<Self::Image, Self::Error> {
            Ok(source % self.0)
        }

        fn kernel(&self) -> Self::Kernel {
            "integers equal modulo n"
        }
    }

    #[test]
    fn fiber_membership_is_defined_by_projection_image() {
        let projection = Modulo(3);
        let fiber = Fiber::of(&projection, &4).unwrap();

        assert_eq!(fiber.image(), &1);
        assert!(fiber.contains(&7).unwrap());
        assert!(!fiber.contains(&8).unwrap());
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Row {
        projection: Modulo,
        image: u32,
        kernel: &'static str,
    }

    impl Row {
        fn new(value: u32, projection: Modulo) -> Self {
            Self {
                projection,
                image: projection.project(&value).unwrap(),
                kernel: projection.kernel(),
            }
        }
    }

    impl Projected<u32> for Row {
        type Projection = Modulo;
        type Image = u32;
        type Kernel = &'static str;

        fn projection(&self) -> &Self::Projection {
            &self.projection
        }

        fn image(&self) -> &Self::Image {
            &self.image
        }

        fn kernel(&self) -> &Self::Kernel {
            &self.kernel
        }
    }

    #[test]
    fn projected_values_can_expose_fiber_without_storing_it() {
        let row = Row::new(10, Modulo(4));
        let fiber = row.fiber();

        assert_eq!(fiber.image(), &2);
        assert!(fiber.contains(&14).unwrap());
        assert!(!fiber.contains(&15).unwrap());
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Producer(&'static str);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Subject(&'static str);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Source(&'static str);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Resolver;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ExampleProvenance {
        producer: Producer,
        subject: Subject,
        source: Source,
    }

    impl Provenance for ExampleProvenance {
        type Producer = Producer;
        type Subject = Subject;

        fn producer(&self) -> &Self::Producer {
            &self.producer
        }

        fn subject(&self) -> &Self::Subject {
            &self.subject
        }
    }

    impl VerifiableWith<Resolver> for ExampleProvenance {
        type Error = std::convert::Infallible;

        fn verify_with(&self, _: &Resolver) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl ResolveWith<Resolver> for ExampleProvenance {
        type Source = Source;

        fn resolve_with(&self, _: &Resolver) -> Result<Self::Source, Self::Error> {
            Ok(self.source.clone())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct WitnessedRow {
        provenance: ExampleProvenance,
    }

    impl Witnessed for WitnessedRow {
        type Provenance = ExampleProvenance;

        fn provenance(&self) -> &Self::Provenance {
            &self.provenance
        }
    }

    #[test]
    fn witnessed_values_are_independent_from_projected_values() {
        let row = WitnessedRow {
            provenance: ExampleProvenance {
                producer: Producer("runtime-1"),
                subject: Subject("record-1"),
                source: Source("payload"),
            },
        };

        let resolver = Resolver;
        row.provenance().verify_with(&resolver).unwrap();
        assert_eq!(
            row.provenance().resolve_with(&resolver).unwrap(),
            Source("payload")
        );
    }
}
