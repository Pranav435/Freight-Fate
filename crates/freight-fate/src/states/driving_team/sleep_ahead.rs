//! A slept-through shift, simulated (owner design, 2026-10-18): the player
//! should not wait while the co-driver drives, so the open road between the
//! things that need them is worked out at once instead of played.
//!
//! Each jump runs the cruise model along the route (the posted limit, the
//! highway fuel burn, the co-driver's logbook) and stops short of the next
//! thing the co-driver acts on: the stop they are making for, the
//! receiver's gate, a chain-law area, or the mile their fuel, break or
//! shift calls for a stop. That last stretch, the stop's screens and the
//! merge back onto the road run as usual, sped up with the sound down.

use ff_core::models::co_driver;
use ff_core::sim::hos::HosClock;

use crate::app::{GameContext, TRANSCRIPT_TARGET};
use crate::states::driving::DrivingState;
use crate::states::driving_core::*;

use super::pilot::FUEL_STOP_FRACTION;
use super::TeamMode;

/// The walk's step along the route.
const STEP_MI: f64 = 0.25;
/// A jump shorter than this is left to the live drive.
const MIN_JUMP_MI: f64 = 2.0;
/// Where a jump ends ahead of the stop the co-driver is making for, or the
/// receiver's exit, beyond the exit window they signal in.
const STOP_LEAD_MI: f64 = 1.0;
/// Where a jump ends ahead of the receiver, beyond the gate's warning.
const GATE_LEAD_MI: f64 = 3.0;
/// The share of the posted limit the cruise holds over a long stretch.
const CRUISE_SHARE: f64 = 0.96;
/// The slowest the cruise model drives a stretch.
const FLOOR_MPH: f64 = 25.0;

/// What a jump covered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Jump {
    pub miles: f64,
    pub minutes: f64,
}

impl DrivingState {
    /// Whether the truck is on the open road at speed with nothing the
    /// co-driver is in the middle of: the only place a jump may start.
    fn sleep_jump_ready(&self, ctx: &GameContext) -> bool {
        let sleeping = self
            .team
            .as_ref()
            .and_then(|team| team.wheel.as_ref())
            .is_some_and(|wheel| wheel.mode == TeamMode::SleepThrough && wheel.summon.is_none());
        sleeping
            && ctx.co_driver_fast_forward
            && self.phase == DRIVE_PHASE_DELIVERY
            && !self.trip.finished
            && !self.trip.on_ramp
            && self.ramp_mi.is_none()
            && self.ramp_stop.is_none()
            && !self.exit_signal_on
            && self.pull_over.is_none()
            && self.hazard_deadline.is_none()
            && !self.chain_checkpoint_hold
            && !self.departure_chain
            && !self.trip.truck.parking_brake
            && self.decel_lane_brake <= 0.0
            && self.trip.truck.speed_mph() >= 40.0
            && (self.keeper_mph.is_some() || self.cruise_mph.is_some())
    }

    /// The mile a jump may run to: short of the next thing the co-driver
    /// acts on. None when nothing allows one.
    fn sleep_jump_limit(&mut self, ctx: &mut GameContext) -> Option<f64> {
        // Off the highway for the receiver: the streets are driven live.
        if self.destination_exit_taken {
            return None;
        }
        let here = self.trip.position_mi;
        let window = self.exit_window_mi();
        let mut limit = self.trip.total_miles() - self.gate_warning_window_mi() - GATE_LEAD_MI;
        // No receiver's exit ahead means the truck is at it, or past it
        // and looping back: driven live.
        let exit = self.destination_exit_stop(ctx)?;
        limit = limit.min(exit.at_mi - window - STOP_LEAD_MI);
        let making_for = self
            .team
            .as_ref()
            .and_then(|team| team.wheel.as_ref())
            .and_then(|wheel| wheel.pilot.making_for.as_ref())
            .map(|(key, _)| key.clone());
        for key in making_for.iter().chain(self.selected_stop_key.iter()) {
            if let Some(stop) = self.trip.stops.iter().find(|stop| &stop.key() == key) {
                if stop.at_mi > here {
                    limit = limit.min(stop.at_mi - window - STOP_LEAD_MI);
                }
            }
        }
        // A chain-law area is the checkpoint's to judge, live.
        let mut mile = here;
        while mile < limit {
            if self.trip.chain_law_area_at(mile).is_some() {
                limit = mile - window;
                break;
            }
            mile += STEP_MI;
        }
        (limit - here >= MIN_JUMP_MI).then_some(limit)
    }

    /// Walk the cruise model from the truck to `limit`, stopping where a
    /// fuel stop, the break or the shift's end first becomes due. Changes
    /// nothing; a need already due at the start does not stop it, since the
    /// co-driver has already planned for it or will hand back.
    fn sleep_jump_walk(&mut self, ctx: &GameContext, limit: f64) -> Jump {
        let mode = ctx.settings.hos_mode.clone();
        let mut hos: HosClock = hos_of(ctx).clone();
        let truck = &self.trip.truck;
        let gal_per_mi = truck.cruise_gal_per_mile();
        let tank = truck.specs.fuel_tank_gal.max(1.0);
        let mut fuel = truck.fuel_gal;
        let fuel_due_at_start = fuel / tank < FUEL_STOP_FRACTION;
        let break_due_at_start = co_driver::break_wanted(&hos, &mode);
        let shift_due_at_start = co_driver::shift_over(&hos, &mode);
        let start = self.trip.position_mi;
        let mut mile = start;
        let mut minutes = 0.0;
        while mile < limit {
            let step = STEP_MI.min(limit - mile);
            let (posted, _) = self.trip.speed_limit_at(mile);
            let mph = (posted * CRUISE_SHARE).max(FLOOR_MPH);
            let step_min = step / mph * 60.0;
            hos.drive(step_min);
            fuel -= step * gal_per_mi;
            mile += step;
            minutes += step_min;
            let due = (!fuel_due_at_start && fuel / tank < FUEL_STOP_FRACTION)
                || (!break_due_at_start && co_driver::break_wanted(&hos, &mode))
                || (!shift_due_at_start && co_driver::shift_over(&hos, &mode));
            if due {
                break;
            }
        }
        Jump {
            miles: mile - start,
            minutes,
        }
    }

    /// Run the open road ahead at once, while the player sleeps. Returns the
    /// jump made, if any.
    pub fn sleep_ahead(&mut self, ctx: &mut GameContext) -> Option<Jump> {
        if !self.sleep_jump_ready(ctx) {
            return None;
        }
        let limit = self.sleep_jump_limit(ctx)?;
        let jump = self.sleep_jump_walk(ctx, limit);
        if jump.miles < MIN_JUMP_MI {
            return None;
        }
        let to = self.trip.position_mi + jump.miles;
        log::info!(
            target: TRANSCRIPT_TARGET,
            "[co-driver] drives {:.1} miles in {:.0} minutes while you sleep",
            jump.miles,
            jump.minutes
        );
        // The clocks: the co-driver drove it, the player slept it.
        self.trip.game_minutes += jump.minutes;
        self.trip.weather.update(jump.minutes);
        hos_mut_of(ctx).drive(jump.minutes);
        self.trip.last_moved_mi = jump.miles;
        self.update_team_hours(ctx, jump.minutes);
        self.trip.last_moved_mi = 0.0;
        // The truck: the miles on the odometer, at the highway burn.
        self.trip.truck.odometer_mi += jump.miles;
        self.trip.truck.burn_cruise_fuel_over_miles(jump.miles);
        // The road: tolls passed are charged, everything else behind the
        // truck is history, and the traffic is the traffic here.
        self.trip.position_mi = to;
        self.trip.check_tolls();
        let minutes = self.trip.game_minutes;
        self.trip.restore(to, minutes);
        self.trip.set_npc_vehicles(Vec::new());
        Some(jump)
    }
}
