//! What the co-driver does at a stop, without the stop's menu (owner,
//! 2026-10-18): fuel and their 30-minute break happen as a sound and a line
//! in the message log, then they drive on. Only the end of their shift opens
//! the menu, because that is where the player takes over.

use ff_core::message_log::MessageCategory;
use ff_core::models::loyalty::loyalty_earnings_text;
use ff_core::sim::trip_models::RoadStop;

use crate::app::{GameContext, Say, TRANSCRIPT_TARGET};
use crate::states::driving::DrivingState;
use crate::states::driving_core::*;

use super::overlay::TOP_UP_FRACTION;
use super::pilot::StopPurpose;
use super::TeamMode;

impl DrivingState {
    /// Fill the tank at `stop` the way the pump does. Returns the gallons.
    pub(crate) fn co_driver_refuel(&mut self, ctx: &mut GameContext, stop: &RoadStop) -> f64 {
        let truck = &self.trip.truck;
        let mut need = truck.specs.fuel_tank_gal - truck.fuel_gal;
        if need < 1.0 {
            return 0.0;
        }
        let region = self.trip.current_region().to_string();
        if player_pays_operating_costs(&profile_of(ctx).business_status) {
            let mut cost = ctx.economy.fuel_cost(&region, need) + 35.0;
            let money = profile_of(ctx).money();
            if money < cost {
                need = ((money - 35.0) / ctx.economy.fuel_price(&region)).max(0.0);
                if need < 5.0 {
                    log::info!(
                        target: TRANSCRIPT_TARGET,
                        "[co-driver] cannot afford fuel at {}",
                        stop.spoken_name()
                    );
                    return 0.0;
                }
                cost = ctx.economy.fuel_cost(&region, need) + 35.0;
            }
            profile_mut_of(ctx).spend(cost);
        }
        self.trip.truck.refuel(Some(need));
        self.stop_visit(stop).fueled = true;
        advance_rest_clock(self, ctx, FUEL_STOP_MIN, None, "");
        hos_mut_of(ctx).on_duty(FUEL_STOP_MIN);
        ctx.audio.play("vehicle/fuel_pump");
        let result = profile_mut_of(ctx)
            .loyalty
            .add_fueling(need, None, &stop.name, &region);
        log::info!(
            target: TRANSCRIPT_TARGET,
            "[co-driver] fuels {need:.0} gallons at {}. {}",
            stop.spoken_name(),
            loyalty_earnings_text(need, result.points_earned, &result.rewards)
        );
        if let Some(wheel) = self.team.as_mut().and_then(|team| team.wheel.as_mut()) {
            wheel.log.fuel_stops.push(stop.spoken_name());
        }
        need
    }

    /// The co-driver's 30-minute break at `stop`.
    pub(crate) fn co_driver_break(&mut self, ctx: &mut GameContext, stop: &RoadStop) {
        advance_rest_clock(self, ctx, 30.0, None, "");
        hos_mut_of(ctx).take_break(30.0);
        log::info!(
            target: TRANSCRIPT_TARGET,
            "[co-driver] takes a 30-minute break at {}",
            stop.spoken_name()
        );
    }

    /// The co-driver pulled into `stop`: fuel and the break are done here
    /// with no menu, the stop goes in the message log, and they drive on.
    /// False when the stop's menu should open instead: the end of their
    /// shift, or a scale.
    pub fn co_driver_works_stop(&mut self, ctx: &mut GameContext, stop: &RoadStop) -> bool {
        let Some(team) = self.team.as_ref() else {
            return false;
        };
        let name = team.co_driver.name;
        let Some(wheel) = team.wheel.as_ref() else {
            return false;
        };
        if wheel.pilot.shift_over || stop.stop_type == "weigh_station" {
            return false;
        }
        let mode = wheel.mode;
        let making_for = wheel.pilot.making_for.clone();
        let purpose = making_for.as_ref().map(|(_, purpose)| *purpose);
        advance_rest_clock(self, ctx, STOP_PULL_IN_MIN, None, "");
        hos_mut_of(ctx).on_duty(STOP_PULL_IN_MIN);
        let sells_fuel = stop.actions.iter().any(|action| action == "fuel");
        let fuel = self.trip.truck.fuel_fraction();
        let wants_fuel =
            fuel < TOP_UP_FRACTION || (purpose == Some(StopPurpose::Fuel) && fuel < 0.98);
        let gallons = if sells_fuel && wants_fuel {
            self.co_driver_refuel(ctx, stop)
        } else {
            0.0
        };
        let breaks_here = stop.actions.iter().any(|action| action == "break");
        let took_break = purpose == Some(StopPurpose::Break) && breaks_here;
        if took_break {
            self.co_driver_break(ctx, stop);
        }
        let place = stop.spoken_name();
        let line = match (gallons > 0.0, took_break) {
            (true, true) => format!(
                "{name} fueled {} gallons and took a 30-minute break at {place}.",
                gallons.round()
            ),
            (true, false) => format!("{name} fueled {} gallons at {place}.", gallons.round()),
            (false, true) => format!("{name} took a 30-minute break at {place}."),
            (false, false) => format!("{name} stopped at {place}."),
        };
        ctx.message_log.add(&line, MessageCategory::Event);
        if mode == TeamMode::RideAlong {
            // The player is awake in the passenger seat: one line, after
            // whatever is speaking.
            ctx.player_asked(|ctx| ctx.say_with(line, Say::queued()));
        }
        // The stop is done with: none of the plan is left to pull in again.
        if let Some(wheel) = self.team.as_mut().and_then(|team| team.wheel.as_mut()) {
            wheel.pilot.making_for = None;
        }
        let key = stop.key();
        let ours = |planned: &String| {
            *planned == key || making_for.as_ref().is_some_and(|(plan, _)| plan == planned)
        };
        if self.trip.planned_stop_key.as_ref().is_some_and(ours) {
            self.trip.planned_stop_key = None;
        }
        if self.selected_stop_key.as_ref().is_some_and(ours) {
            self.clear_selected_stop_intent();
        }
        true
    }
}
