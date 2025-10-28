use crate::inline::{InlineString, TypeSize};
use crate::{FixedString, ValidLength};
use core::any::type_name;
use core::error::Error;
use core::fmt::{write, Debug, Display, Write};
use core::marker::PhantomData;

pub trait TryToFixedString {
    /// Tries to convert the given value to a `FixedString`.
    ///
    /// Should get the same result as `.to_string().try_into()`
    ///
    /// # Errors
    ///
    /// It fails if the string represantation of
    fn try_to_fixed_string<T: ValidLength>(&self) -> Result<FixedString<T>, CouldNotFitStringError<T>>;
}

impl<D: Display + ?Sized> TryToFixedString for D {
    fn try_to_fixed_string<T: ValidLength>(&self) -> Result<FixedString<T>, CouldNotFitStringError<T>> {
        let mut builder = FixedStringBuilder::<T>::new();

        let () = write!(builder, "{self}").map_err(|_| CouldNotFitStringError { _x: PhantomData })?;

        Ok(builder.into_fixed_string())
    }
}

pub struct CouldNotFitStringError<L: ValidLength> {
    _x: PhantomData<L>
}

impl<L: ValidLength> Debug for CouldNotFitStringError<L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CouldNotFitStringError")
            .field("max_len", &L::MAX.to_usize())
            .finish_non_exhaustive()
    }
}

impl<L: ValidLength> Display for CouldNotFitStringError<L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FixedString<{}> can be at most {} long", type_name::<L>(), L::MAX)
    }
}

impl<L: ValidLength> Error for CouldNotFitStringError<L> {}


enum StringData<T: ValidLength> {
    Stack(InlineString<T::InlineStrRepr>),
    /// Must be at most `T::MAX` long
    Heap(String),
}

struct FixedStringBuilder<T: ValidLength> {
    data: StringData<T>,
}

impl<T: ValidLength> FixedStringBuilder<T> {
    fn new() -> Self {
        Self {
            data: StringData::Stack(InlineString::empty()),
        }
    }

    fn len(&self) -> usize {
        match &self.data {
            StringData::Stack(inline_string) => inline_string.len().into(),
            StringData::Heap(string) => string.len(),
        }
    }

    fn into_fixed_string(self) -> FixedString<T> {
        match self.data {
            StringData::Stack(inline_string) => FixedString::new_inline(inline_string.as_str()),
            StringData::Heap(string) => string.try_into().ok(),
        }.expect("Is guaranteed to fit")
    }

    fn max_stack_len() -> usize {
        InlineString::<T::InlineStrRepr>::max_len()
    }

    fn max_heap_len() -> usize {
        T::MAX.to_usize()
    }

    fn push_str(&mut self, s: &str) -> Option<()> {
        let current_length = self.len();

        let total_length = current_length.checked_add(s.len())?;

        if total_length <= Self::max_stack_len() {
            let StringData::Stack(array) = self.data else {
                unreachable!("The string can't get shorter and we only put on heap if it's too long for stack.");
            };

            let Some(array) = InlineString::from_len_and_write(total_length, |arr| {
                arr[..current_length].copy_from_slice(array.as_str().as_bytes());
                arr[current_length..total_length].copy_from_slice(s.as_bytes());
            }) else {
                unreachable!("Length is checked above.");
            };

            self.data = StringData::Stack(array);
            Some(())
        } else if total_length <= Self::max_heap_len() {
            if let StringData::Stack(inline_string) = &self.data {
                self.data = StringData::Heap(inline_string.as_str().into());
            }

            let StringData::Heap(string) = &mut self.data else {
                unreachable!("self.data is StringData::Heap.");
            };

            string.push_str(s);

            Some(())
        } else {
            None
        }
    }
}

impl<T: ValidLength> Write for FixedStringBuilder<T> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.push_str(s).ok_or(core::fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Display;

    use crate::{FixedString, ValidLength};

    use super::TryToFixedString;


    fn assert_try_to_string_generic<T: ValidLength>(value: &(impl Display + ?Sized)) {
        assert_eq!(
            value.try_to_fixed_string::<T>().ok(),
            value.to_string().try_into().ok(),
            "Converting {value} to FixedString<{}> failed",
            core::any::type_name::<T>(),
        );
    }

    fn assert_try_to_string(value: &(impl Display + ?Sized)) {
        assert_try_to_string_generic::<u8>(value);
        assert_try_to_string_generic::<u16>(value);
        #[cfg(any(target_pointer_width = "64", target_pointer_width = "32"))]
        assert_try_to_string_generic::<u32>(value);
    }

    #[test]
    fn test_converting_strings() {
        assert_try_to_string("");
        assert_try_to_string("Hello, world!");
    }

    #[test]
    fn test_converting_numbers() {
        assert_try_to_string(&1);
        assert_try_to_string(&i32::MIN);
        assert_try_to_string(&u64::MAX);
        assert_try_to_string(&i128::MIN);
        assert_try_to_string(&u128::MAX);
    }

    fn assert_is_inline(str: FixedString::<u8>) {
        assert!(str.is_inline(), "{str} should be inline");
        std::mem::drop(str);
    }

    #[test]
    fn test_try_to_string_avoids_heap_allocations() {
        assert_is_inline("".try_to_fixed_string().unwrap());
        assert_is_inline('🦀'.try_to_fixed_string().unwrap());
        assert_is_inline(2000.try_to_fixed_string().unwrap());
    }
}
