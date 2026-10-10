use crate::format;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, NaiveDate, SingleId};
use erp_search_code_gen::make_domain;

#[selection]
pub enum SequenceReset {
    #[selection(label = "Never")]
    Never,
    #[default]
    #[selection(label = "Every year")]
    Yearly,
    #[selection(label = "Every month")]
    Monthly,
}

/// A series of document numbers: `INV/{year}/` followed by the next number, padded.
#[derive(Model)]
#[erp(id = "sequence", methods)]
#[allow(dead_code)]
pub struct Sequence<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(
        description = "What the code asking for a number names the series by",
        index
    )]
    code: String,
    #[erp(description = "Placeholders: {year}, {y}, {month}, {day}")]
    prefix: Option<String>,
    suffix: Option<String>,
    #[erp(
        default = 5,
        description = "The number is padded with zeros to this many digits"
    )]
    padding: i32,
    #[erp(label = "Next number", default = 1)]
    number_next: i32,
    #[erp(label = "Step", default = 1)]
    number_increment: i32,
    #[erp(label = "Restart numbering")]
    reset: SequenceReset,
    #[erp(private)]
    period: Option<String>,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl Sequence<SingleId> {
    /// The active series named `code`, empty when there is none.
    pub fn by_code(env: &mut Environment, code: String) -> Result<Sequence<SingleId>> {
        let env = &mut *env.sudo();
        let found: Sequence<MultipleIds> =
            env.search(&make_domain!([("code", "=", code), ("active", "=", true)]))?;
        Ok(match found.into_iter().next() {
            Some(sequence) => sequence,
            None => env.get_record(SingleId::empty()),
        })
    }

    /// The next name of the series `code`, for a document dated `date`.
    ///
    /// Errs when no active series has that code: a document without a number is no document.
    pub fn next_by_code(env: &mut Environment, code: String, date: NaiveDate) -> Result<String> {
        let sequence = Self::by_code(env, code.clone())?;
        if sequence.is_empty() {
            return Err(format!("No numbering is set up for \"{code}\"").into());
        }
        sequence.next(env, date)
    }

    /// The next `count` names of the series `code`, for documents dated `date`, reserved at once.
    ///
    /// Errs when no active series has that code.
    pub fn next_many_by_code(
        env: &mut Environment,
        code: String,
        date: NaiveDate,
        count: usize,
    ) -> Result<Vec<String>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let sequence = Self::by_code(env, code.clone())?;
        if sequence.is_empty() {
            return Err(format!("No numbering is set up for \"{code}\"").into());
        }
        sequence.next_many(env, date, count)
    }

    /// The next name of this series for a document dated `date`, the series moved on.
    ///
    /// The series is locked first, until the transaction ends: two documents numbered at once
    /// wait for each other, rather than both take the same number.
    ///
    /// As sudo: whoever may create the document may number it, without managing the series.
    pub fn next(&self, env: &mut Environment, date: NaiveDate) -> Result<String> {
        let mut names = self.next_many(env, date, 1)?;
        Ok(names.pop().unwrap_or_default())
    }

    /// The next `count` names of this series for documents dated `date`, the series moved on past
    /// all of them in one write.
    ///
    /// Locked and as sudo, as [`Sequence::next`].
    pub fn next_many(
        &self,
        env: &mut Environment,
        date: NaiveDate,
        count: usize,
    ) -> Result<Vec<String>> {
        let env = &mut *env.sudo();
        env.lock_records("sequence", &SingleId::from(self.get_id()))?;
        let reset = *self.get_reset(env)?;
        let period = format::period(
            date,
            matches!(reset, SequenceReset::Yearly),
            matches!(reset, SequenceReset::Monthly),
        );
        let mut number = *self.get_number_next(env)?;
        if self
            .get_period(env)?
            .map(String::as_str)
            .unwrap_or_default()
            != period
        {
            number = 1;
            self.set_period(Some(period), env)?;
        }
        let step = (*self.get_number_increment(env)?).max(1);
        let count = i32::try_from(count)?;
        self.set_number_next(number + step * count, env)?;
        let prefix = self.get_prefix(env)?.cloned().unwrap_or_default();
        let suffix = self.get_suffix(env)?.cloned().unwrap_or_default();
        let padding = *self.get_padding(env)?;
        Ok((0..count)
            .map(|at| format::format_name(&prefix, number + step * at, padding, &suffix, date))
            .collect())
    }
}

#[erp_methods]
impl Sequence<MultipleIds> {
    /// One active series per code; numbers and steps are positive; padding is within reason.
    #[erp(check = ["code", "number_next", "number_increment", "padding", "active"])]
    pub fn check_series(&self, env: &mut Environment) -> Result<()> {
        for sequence in self {
            let code = sequence.get_code(env)?.clone();
            if *sequence.get_number_next(env)? < 1 || *sequence.get_number_increment(env)? < 1 {
                return Err(format!(
                    "The numbering \"{code}\" must start at 1 or more and step forward"
                )
                .into());
            }
            if !(0..=20).contains(sequence.get_padding(env)?) {
                return Err(format!("The padding of \"{code}\" must be between 0 and 20").into());
            }
            let same = env.count(
                "sequence",
                &make_domain!([("code", "=", code.clone()), ("active", "=", true)]),
            )?;
            if same > 1 {
                return Err(format!("Another active numbering already uses \"{code}\"").into());
            }
        }
        Ok(())
    }
}
