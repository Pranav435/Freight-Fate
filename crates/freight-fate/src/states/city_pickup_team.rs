//! Leaving the shipper with the co-driver at the wheel.

use ff_core::models::co_driver;

use crate::app::GameContext;
use crate::states::base::MenuItem;
use crate::states::city::profile;
use crate::states::city_pickup::PickupFacilityState;
use crate::states::driving_team::handover::{job_pairing, terms_text};
use crate::states::driving_team::TeamMode;

impl PickupFacilityState {
    /// Whether the dock offers a team departure: a team load, or one long
    /// enough that driving it alone needs a 10-hour break.
    pub(crate) fn team_departure_offered(&self, ctx: &GameContext) -> bool {
        if !self.loaded {
            return false;
        }
        if self.job.team {
            return true;
        }
        let route = ctx
            .world
            .supported_route(&self.job.origin, &self.job.destination, None)
            .ok()
            .flatten();
        let drive_min = match route {
            Some(route) => {
                ff_core::models::jobs::route_drive_hours(Some(&route), 0.0, Some(ctx.world)) * 60.0
            }
            None => self.job.distance_mi / ff_core::models::jobs::DEADLINE_AVG_MPH * 60.0,
        };
        co_driver::team_worthwhile(&profile(ctx).hos, &ctx.settings.hos_mode, drive_min)
    }

    /// The two team rows, placed after Depart for destination.
    pub(crate) fn team_departure_rows(&self, ctx: &GameContext) -> Vec<MenuItem<Self>> {
        if !self.team_departure_offered(ctx) {
            return Vec::new();
        }
        let pairing = job_pairing(ctx, &self.job);
        let name = pairing.co_driver.name;
        let carrier = ff_core::models::career::carrier_name_of(profile(ctx));
        let who = co_driver::pairing_line(&pairing, &carrier);
        let terms = terms_text(ctx, name);
        vec![
            MenuItem::new(
                format!("Depart with {name} driving, and ride along"),
                |s: &mut Self, ctx| s.depart_as_team(ctx, TeamMode::RideAlong),
            )
            .help(format!(
                "{who} You rest in the sleeper and hear the road. {terms}"
            )),
            MenuItem::new(
                format!("Depart with {name} driving, and sleep"),
                |s: &mut Self, ctx| s.depart_as_team(ctx, TeamMode::SleepThrough),
            )
            .help(format!(
                "{who} You wake when {name} needs you, with a summary of the drive. {terms}"
            )),
        ]
    }

    fn depart_as_team(&mut self, ctx: &mut GameContext, mode: TeamMode) {
        self.team_departure = Some(mode);
        self.depart_for_destination(ctx);
    }
}
