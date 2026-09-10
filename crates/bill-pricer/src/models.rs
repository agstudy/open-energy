use rust_decimal::Decimal;
use bitflags::bitflags;

pub struct HourMinute {
    pub hour: u8,
    pub minute: u8,
}

bitflags! {

    pub struct WeekDays: u8 {

        const MON = 0b0000001;
        const TUE = 0b0000010;
        const WED = 0b0000100;
        const THU = 0b0001000;
        const FRI = 0b0010000;
        const SAT = 0b0100000;
        const SUN = 0b1000000;
    }
}

pub struct TimeBand {
    pub start: HourMinute,
    pub end: HourMinute,
    pub days_of_week: Option<Vec<WeekDays>>
}

pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Option<Decimal>,
}

pub enum DiscountValueType {
    Percent,
    Absolute,
}

pub enum DiscountApplicationType {
    Total,
    Energy,
}

pub struct Discount {
    pub value_type: DiscountValueType,
    pub application_type: DiscountApplicationType,
    pub amount: Decimal,
}

pub struct RateSchedule {
    pub rates: Vec<RateBlock>,
    pub times: Vec<TimeBand>,
}


pub struct Tariff {
    pub usage_rate: Vec<RateSchedule>,
    pub export_rate: Option<Vec<RateSchedule>>,
    pub discount: Option<Vec<Discount>>,
    pub supply_rate: Decimal,
}

