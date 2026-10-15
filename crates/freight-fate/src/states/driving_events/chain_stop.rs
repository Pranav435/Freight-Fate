//! Roadside chain work: putting snow chains on and taking them off,
//! tightening a slapping chain and fixing a broken one. The pause menu and
//! facility stopping assistance both call the same `do_*` actions, so the
//! work, the time and the lines are the same whoever starts it.

use ff_core::pyfmt::fmt_f;

use crate::app::GameContext;
use crate::states::driving::DrivingState;
use crate::states::driving_core::{
    advance_rest_clock, clock_text, deadline_text, hos_mut_of, is_night, profile_mut_of,
    profile_of, CHAIN_INSTALL_FATIGUE, CHAIN_INSTALL_MIN, CHAIN_INSTALL_NIGHT_FATIGUE,
    CHAIN_INSTALL_NIGHT_MULT, CHAIN_REMOVE_FATIGUE, CHAIN_REMOVE_MIN, CHAIN_REPAIR_MIN,
    CHAIN_REPAIR_WEAR_RESTORE, CHAIN_SAFE_MPH, CHAIN_TIGHTEN_MIN, CHAIN_TIGHTEN_WEAR_RESTORE,
};

impl DrivingState {
    /// Put the chains on at a stop (on duty), or None after "Come to a complete stop first."
    /// Shared by the pause menu and facility stopping assistance.
    pub fn do_install_chains(&mut self, ctx: &mut GameContext) -> Option<String> {
        if self.trip.truck.speed_mph() > 3.0 {
            ctx.say("Come to a complete stop first.");
            return None;
        }
        let night = is_night(self.trip.local_hour());
        let minutes = CHAIN_INSTALL_MIN * if night { CHAIN_INSTALL_NIGHT_MULT } else { 1.0 };
        let fatigue = if night {
            CHAIN_INSTALL_NIGHT_FATIGUE
        } else {
            CHAIN_INSTALL_FATIGUE
        };
        advance_rest_clock(self, ctx, minutes, Some("on_duty_not_driving"), "chain up");
        hos_mut_of(ctx).on_duty(minutes);
        {
            let p = profile_mut_of(ctx);
            p.fatigue = (p.fatigue + fatigue).min(100.0);
        }
        self.trip.truck.chains_on = true;
        self.chains_fast_active = false;
        self.chain_slap_said = false;
        self.chain_tightened = false;
        ctx.audio.play("ui/notify");
        let effort = if night {
            "In the dark, it took more out of you. "
        } else {
            ""
        };
        let bare = if self.trip.truck.surface != "snow" && self.trip.truck.surface != "ice" {
            " The road here is bare. Chains wear out fast until you reach snow."
        } else {
            ""
        };
        let text = format!(
            "Chains on in {} minutes. {effort}Keep under {}.{bare} It is {}. {}",
            fmt_f(minutes, 0),
            ctx.settings.speed_text(CHAIN_SAFE_MPH),
            clock_text(self.trip.local_hour()),
            deadline_text(self, ctx)
        );
        Some(text)
    }

    /// Fix a broken chain with a spare link (on duty).
    /// Shared by the pause menu and facility stopping assistance.
    pub fn do_repair_chain(&mut self, ctx: &mut GameContext) -> Option<String> {
        if self.trip.truck.speed_mph() > 3.0 {
            ctx.say("Come to a complete stop first.");
            return None;
        }
        let links = profile_of(ctx).chain_repair_links();
        if links <= 0 {
            ctx.audio.play("ui/error");
            return Some("No repair links left. The chain stays off.".to_string());
        }
        advance_rest_clock(
            self,
            ctx,
            CHAIN_REPAIR_MIN,
            Some("on_duty_not_driving"),
            "repair chain",
        );
        hos_mut_of(ctx).on_duty(CHAIN_REPAIR_MIN);
        profile_mut_of(ctx).set_chain_repair_links(links - 1);
        self.trip.truck.chain_wear_pct = 100.0 - CHAIN_REPAIR_WEAR_RESTORE;
        self.trip.truck.chains_on = true;
        self.chains_fast_active = false;
        self.chain_slap_said = false;
        self.chain_tightened = false;
        ctx.audio.play("ui/notify");
        Some(format!(
            "Chain repaired with a spare link. Back on in {} minutes. Check it again soon. \
             It is {}. {}",
            fmt_f(CHAIN_REPAIR_MIN, 0),
            clock_text(self.trip.local_hour()),
            deadline_text(self, ctx)
        ))
    }

    /// Take up the slack in a slapping chain (on duty).
    /// Shared by the pause menu and facility stopping assistance.
    pub fn do_tighten_chains(&mut self, ctx: &mut GameContext) -> Option<String> {
        if self.trip.truck.speed_mph() > 3.0 {
            ctx.say("Come to a complete stop first.");
            return None;
        }
        advance_rest_clock(
            self,
            ctx,
            CHAIN_TIGHTEN_MIN,
            Some("on_duty_not_driving"),
            "tighten chains",
        );
        hos_mut_of(ctx).on_duty(CHAIN_TIGHTEN_MIN);
        self.trip.truck.chain_wear_pct =
            (self.trip.truck.chain_wear_pct - CHAIN_TIGHTEN_WEAR_RESTORE).max(0.0);
        self.chain_tightened = true;
        ctx.audio.play("ui/notify");
        Some(format!(
            "Chains tightened in {} minutes. It is {}. {}",
            fmt_f(CHAIN_TIGHTEN_MIN, 0),
            clock_text(self.trip.local_hour()),
            deadline_text(self, ctx)
        ))
    }

    /// Take the chains off at a stop (on duty).
    /// Shared by the pause menu and facility stopping assistance.
    pub fn do_remove_chains(&mut self, ctx: &mut GameContext) -> Option<String> {
        if self.trip.truck.speed_mph() > 3.0 {
            ctx.say("Come to a complete stop first.");
            return None;
        }
        advance_rest_clock(
            self,
            ctx,
            CHAIN_REMOVE_MIN,
            Some("on_duty_not_driving"),
            "remove chains",
        );
        hos_mut_of(ctx).on_duty(CHAIN_REMOVE_MIN);
        {
            let p = profile_mut_of(ctx);
            p.fatigue = (p.fatigue + CHAIN_REMOVE_FATIGUE).min(100.0);
        }
        self.trip.truck.chains_on = false;
        ctx.audio.play("ui/notify");
        let wear = self.trip.truck.chain_wear_pct;
        let state_word = if wear >= 75.0 {
            "The set is about done.".to_string()
        } else if wear >= 1.0 {
            format!("The set is {} percent worn.", fmt_f(wear, 0))
        } else {
            "The set is still fresh.".to_string()
        };
        let text = format!(
            "Chains off in {} minutes. {state_word} It is {}. {}",
            fmt_f(CHAIN_REMOVE_MIN, 0),
            clock_text(self.trip.local_hour()),
            deadline_text(self, ctx)
        );
        Some(text)
    }
}
