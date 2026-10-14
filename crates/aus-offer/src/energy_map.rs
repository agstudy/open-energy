use std::{collections::BTreeMap, sync::LazyLock};

/// AER-published energy retailer base URIs (January 2026).
/// Key   = retailer name as it appears on the AER list.
/// Value = the correct base URI for that retailer's CDR product data.
/// list is here : https://www.aer.gov.au/system/files/2026-01/Consumer%20Data%20Right%20-%20Energy%20Retailer%20Base%20URIs%20and%20CDR%20Brands%20-%20January%202026.pdf#1#1
pub static ENERGY_BASE_URIS: LazyLock<BTreeMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = BTreeMap::new();

    m.insert(
        "1st Energy",
        "https://cdr.energymadeeasy.gov.au/1st-energy/",
    );
    m.insert("ActewAGL", "https://cdr.energymadeeasy.gov.au/actewagl/");
    m.insert(
        "Active Utilities Retail",
        "https://cdr.energymadeeasy.gov.au/active-utilities/",
    );
    m.insert("AGL", "https://cdr.energymadeeasy.gov.au/agl/");
    m.insert("Alinta Energy", "https://cdr.energymadeeasy.gov.au/alinta/");
    m.insert(
        "Altogether",
        "https://cdr.energymadeeasy.gov.au/altogether/",
    );
    m.insert("Amber Electric", "https://cdr.energymadeeasy.gov.au/amber/");
    m.insert("Ampol Energy", "https://cdr.energymadeeasy.gov.au/ampol/");
    m.insert(
        "Arc Energy Group",
        "https://cdr.energymadeeasy.gov.au/arc-energy/",
    );
    m.insert(
        "ARCLINE by RACV",
        "https://cdr.energymadeeasy.gov.au/energy-locals/",
    );
    m.insert("Arcstream", "https://cdr.energymadeeasy.gov.au/arcstream/");
    m.insert("ASENO", "https://cdr.energymadeeasy.gov.au/aseno/");
    m.insert("Aurora Energy", "https://cdr.energymadeeasy.gov.au/aurora/");
    m.insert("Besy", "https://cdr.energymadeeasy.gov.au/besy/");
    m.insert("Blue NRG", "https://cdr.energymadeeasy.gov.au/blue-nrg/");
    m.insert(
        "Brighte Energy",
        "https://cdr.energymadeeasy.gov.au/brighte/",
    );
    m.insert(
        "CleanCo Queensland",
        "https://cdr.energymadeeasy.gov.au/cleanco/",
    );
    m.insert(
        "CleanPeak Energy Retail",
        "https://cdr.energymadeeasy.gov.au/cleanpeak/",
    );
    m.insert("Coles Energy", "https://cdr.energymadeeasy.gov.au/coles/");
    m.insert(
        "Commander Power & Gas",
        "https://cdr.energymadeeasy.gov.au/commander/",
    );
    m.insert(
        "Cooperative Power",
        "https://cdr.energymadeeasy.gov.au/energy-locals/",
    );
    m.insert("CovaU", "https://cdr.energymadeeasy.gov.au/covau/");
    m.insert(
        "CPE Mascot",
        "https://cdr.energymadeeasy.gov.au/cpe-mascot/",
    );
    m.insert(
        "Diamond Energy",
        "https://cdr.energymadeeasy.gov.au/diamond/",
    );
    m.insert(
        "Discover Energy",
        "https://cdr.energymadeeasy.gov.au/discover/",
    );
    m.insert(
        "Dodo Power & Gas",
        "https://cdr.energymadeeasy.gov.au/dodo/",
    );
    m.insert(
        "Ellis Air Connect",
        "https://cdr.energymadeeasy.gov.au/ea-connect/",
    );
    m.insert(
        "Energy Locals Retail",
        "https://cdr.energymadeeasy.gov.au/energy-locals/",
    );
    m.insert(
        "Energy Locals Urban",
        "https://cdr.energymadeeasy.gov.au/energy-locals-urban/",
    );
    m.insert(
        "EnergyAustralia",
        "https://cdr.energymadeeasy.gov.au/energyaustralia/",
    );
    m.insert("ENGIE", "https://cdr.energymadeeasy.gov.au/engie/");
    m.insert(
        "ERC Energy",
        "https://cdr.energymadeeasy.gov.au/erc-energy/",
    );
    m.insert(
        "Ergon Energy Retail",
        "https://cdr.energymadeeasy.gov.au/ergon/",
    );
    m.insert("Every", "https://cdr.energymadeeasy.gov.au/every/");
    m.insert(
        "Flipped Energy",
        "https://cdr.energymadeeasy.gov.au/flipped/",
    );
    m.insert(
        "Flow Power",
        "https://cdr.energymadeeasy.gov.au/flow-power/",
    );
    m.insert(
        "Future X Power",
        "https://cdr.energymadeeasy.gov.au/future-x/",
    );
    m.insert(
        "GEE Energy",
        "https://cdr.energymadeeasy.gov.au/gee-energy/",
    );
    m.insert(
        "GloBird Energy",
        "https://cdr.energymadeeasy.gov.au/globird/",
    );
    m.insert(
        "Glow Power",
        "https://cdr.energymadeeasy.gov.au/glow-power/",
    );
    m.insert(
        "Humenergy Group",
        "https://cdr.energymadeeasy.gov.au/humenergy/",
    );
    m.insert("iGENO", "https://cdr.energymadeeasy.gov.au/igeno/");
    m.insert("iO Energy", "https://cdr.energymadeeasy.gov.au/radian/");
    m.insert("Kogan Energy", "https://cdr.energymadeeasy.gov.au/kogan/");
    m.insert(
        "Locality Planning Energy",
        "https://cdr.energymadeeasy.gov.au/locality-planning/",
    );
    m.insert(
        "Localvolts",
        "https://cdr.energymadeeasy.gov.au/localvolts/",
    );
    m.insert("Lumo Energy", "https://cdr.energymadeeasy.gov.au/lumo/");
    m.insert(
        "Macarthur Energy Retail",
        "https://cdr.energymadeeasy.gov.au/macarthur/",
    );
    m.insert("Macquarie", "https://cdr.energymadeeasy.gov.au/macquarie/");
    m.insert(
        "Metered Energy Holdings",
        "https://cdr.energymadeeasy.gov.au/metered-energy/",
    );
    m.insert(
        "Microgrid Power",
        "https://cdr.energymadeeasy.gov.au/microgrid/",
    );
    m.insert(
        "Momentum Energy",
        "https://cdr.energymadeeasy.gov.au/momentum/",
    );
    m.insert(
        "MYOB powered by OVO",
        "https://cdr.energymadeeasy.gov.au/ovo-energy/",
    );
    m.insert("Nectr", "https://cdr.energymadeeasy.gov.au/nectr/");
    m.insert(
        "Next Business Energy",
        "https://cdr.energymadeeasy.gov.au/next-business/",
    );
    m.insert("Origin Energy", "https://cdr.energymadeeasy.gov.au/origin/");
    m.insert(
        "OVO Energy",
        "https://cdr.energymadeeasy.gov.au/ovo-energy/",
    );
    m.insert(
        "OVO Energy for CTM",
        "https://cdr.energymadeeasy.gov.au/ovo-energy/",
    );
    m.insert(
        "Pacific Blue Retail",
        "https://cdr.energymadeeasy.gov.au/pacific-blue/",
    );
    m.insert(
        "Perpetual Energy",
        "https://cdr.energymadeeasy.gov.au/perpetual/",
    );
    m.insert("PowerHub", "https://cdr.energymadeeasy.gov.au/powerhub/");
    m.insert("Powershop", "https://cdr.energymadeeasy.gov.au/powershop/");
    m.insert("Powow Power", "https://cdr.energymadeeasy.gov.au/powow/");
    m.insert(
        "RAA Energy",
        "https://cdr.energymadeeasy.gov.au/energy-locals/",
    );
    m.insert("Radian Energy", "https://cdr.energymadeeasy.gov.au/radian/");
    m.insert(
        "Real Utilities",
        "https://cdr.energymadeeasy.gov.au/real-utilities/",
    );
    m.insert(
        "Red Energy",
        "https://cdr.energymadeeasy.gov.au/red-energy/",
    );
    m.insert("Savant Energy", "https://cdr.energymadeeasy.gov.au/savant/");
    m.insert("Seene", "https://cdr.energymadeeasy.gov.au/seene/");
    m.insert(
        "Shell Energy",
        "https://cdr.energymadeeasy.gov.au/shell-energy/",
    );
    m.insert(
        "Silver Asset Energy",
        "https://cdr.energymadeeasy.gov.au/silver-asset/",
    );
    m.insert(
        "Smart Energy",
        "https://cdr.energymadeeasy.gov.au/smart-energy/",
    );
    m.insert(
        "Solstice Energy",
        "https://cdr.energymadeeasy.gov.au/solstice/",
    );
    m.insert(
        "Stanwell Energy",
        "https://cdr.energymadeeasy.gov.au/stanwell/",
    );
    m.insert("Sumo", "https://cdr.energymadeeasy.gov.au/sumo-power/");
    m.insert("Tango Energy", "https://cdr.energymadeeasy.gov.au/tango/");
    m.insert(
        "Telstra Energy",
        "https://cdr.energymadeeasy.gov.au/telstra-energy/",
    );
    m.insert(
        "Tesla Energy Ventures",
        "https://cdr.energymadeeasy.gov.au/tesla/",
    );
    m.insert("Veolia Energy", "https://cdr.energymadeeasy.gov.au/veolia/");
    m.insert(
        "WINconnect",
        "https://cdr.energymadeeasy.gov.au/winconnect/",
    );
    m.insert(
        "YES Energy",
        "https://cdr.energymadeeasy.gov.au/yes-energy/",
    );
    m.insert(
        "ZEN Energy",
        "https://cdr.energymadeeasy.gov.au/zen-energy/",
    );

    m
});

/// Look up the AER base URI by brand name (case-insensitive).
pub fn get_energy_base_uri(brand_name: &str) -> Option<&'static str> {
    ENERGY_BASE_URIS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(brand_name))
        .map(|(_, uri)| *uri)
}
