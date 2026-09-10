use rust_decimal::Decimal;


pub struct HourMinute {
    pub hour: u8,
    pub minute: u8,
}

pub enum WeekDay {
    Sat,Sun,Mon,Tue,Wed,Thu,Fri
}
pub struct TimeBand {
    pub start: HourMinute,
    pub end: HourMinute,
    pub days_of_week: Option<Vec<WeekDay>>
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

