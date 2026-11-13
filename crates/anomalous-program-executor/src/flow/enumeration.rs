//! Immutable nominal enum values. Identity and payload remain inline in the
//! existing bounded operands/locals; no allocation or mutable alias is involved.

use super::{error, program};

#[derive(Clone, Copy)]
pub(super) struct enum_value {
    layout: u8,
    variant: u8,
    payload: Option<i64>,
}

impl enum_value {
    pub(super) fn new(
        program: &program<'_>,
        layout: u8,
        variant: u8,
        payload: Option<i64>,
    ) -> Result<Self, error> {
        let item = Self {
            layout,
            variant,
            payload,
        };
        item.validate(program)?;
        Ok(item)
    }

    pub(super) const fn layout(self) -> u8 {
        self.layout
    }

    fn validate(self, program: &program<'_>) -> Result<(), error> {
        let declared = program
            .enum_layouts()
            .get(usize::from(self.layout))
            .and_then(|layout| layout.variants().get(usize::from(self.variant)))
            .ok_or(error::invalid_state)?;
        if declared.has_payload != self.payload.is_some() {
            return Err(error::invalid_state);
        }
        Ok(())
    }

    pub(super) fn matches(
        self,
        program: &program<'_>,
        layout: u8,
        variant: u8,
    ) -> Result<bool, error> {
        self.validate(program)?;
        if self.layout != layout
            || program
                .enum_layouts()
                .get(usize::from(layout))
                .and_then(|layout| layout.variants().get(usize::from(variant)))
                .is_none()
        {
            return Err(error::invalid_state);
        }
        Ok(self.variant == variant)
    }

    pub(super) fn unwrap(
        self,
        program: &program<'_>,
        layout: u8,
        payload: bool,
    ) -> Result<Option<i64>, error> {
        self.validate(program)?;
        if self.layout != layout {
            return Err(error::invalid_state);
        }
        // Nominal CFG joins deliberately retain all variants of this enum.
        // Exact extraction arity is therefore checked at the point of use.
        if self.payload.is_some() != payload {
            return Err(error::enum_shape);
        }
        Ok(self.payload)
    }
}
