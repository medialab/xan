#[derive(Debug)]
struct Extent<T>((T, T));

impl<T: Copy + PartialOrd> Extent<T> {
    fn constant(value: T) -> Self {
        Self((value, value))
    }

    #[inline]
    fn set_min(&mut self, value: T) {
        self.0 .0 = value;
    }

    #[inline]
    fn set_max(&mut self, value: T) {
        self.0 .1 = value;
    }

    #[inline]
    fn min(&self) -> T {
        self.0 .0
    }

    #[inline]
    fn max(&self) -> T {
        self.0 .1
    }

    fn process(&mut self, value: T) {
        if value < self.0 .0 {
            self.0 .0 = value;
        }

        if value > self.0 .1 {
            self.0 .1 = value;
        }
    }
}

#[derive(Debug)]
struct ExtentBuilder<T>(Option<Extent<T>>);

impl<T: Copy + PartialOrd> ExtentBuilder<T> {
    fn new() -> Self {
        Self(None)
    }

    fn process(&mut self, value: T) {
        match self.0.as_mut() {
            None => {
                self.0 = Some(Extent::constant(value));
            }
            Some(extent) => extent.process(value),
        };
    }
}

#[derive(Debug)]
struct Scale {
    input_domain: (f64, f64),
    output_range: (f64, f64),
}

impl Scale {
    fn new(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain,
            output_range,
        }
    }

    #[inline]
    fn lerp(&self, t: f64) -> f64 {
        (1.0 - t) * self.output_range.0 + t * self.output_range.1
    }

    #[inline]
    fn input_domain_width(&self) -> f64 {
        self.input_domain.1 - self.input_domain.0
    }

    #[inline]
    fn output_range_width(&self) -> f64 {
        self.output_range.1 - self.output_range.0
    }

    #[inline]
    fn percent(&self, value: f64) -> f64 {
        (value - self.input_domain.0) / self.input_domain_width()
    }

    fn map(&self, value: f64) -> f64 {
        let percent = self.percent(value);

        percent * self.output_range_width() + self.output_range.0
    }
}

// TODO: linear, log etc. with different struct to process and one belonging to the scale
// TODO: d3 style nice
// TODO: extent builder with optional custom bounds?
// TODO: convert, invert
// TODO: ticks, continuous or not
