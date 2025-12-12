# Animation System Guide

This guide covers the ploke-tui animation system, including configuration, usage, and creating custom animations.

## Overview

The animation system provides smooth, configurable animations for UI elements in the ploke terminal interface. It supports fade-in, slide-in, typewriter, pulse, and highlight effects with customizable timing and easing functions.

## Architecture

The animation system consists of several key components:

- **`AnimationState`**: Manages all active animations and their lifecycle
- **`AnimationType`**: Defines different animation types (FadeIn, SlideIn, Typewriter, etc.)
- **`AnimationUtils`**: Provides utility functions for applying effects to content
- **`AnimationFactory`**: Creates animation instances with proper configuration
- **`AnimationConfig`**: Configuration struct for animation behavior

## Configuration

### User Configuration

Animations are configured via `~/.config/ploke/config.toml`:

```toml
[animation]
# Master switch - enables/disables all animations
enabled = true

# Default duration for all animations (milliseconds)
default_duration_ms = 300

# Easing function for smooth motion curves
easing_function = "EaseInOut"  # Linear, EaseIn, EaseOut, EaseInOut, Bounce

# Animation effects per message type
new_message_effect = "FadeIn"        # User messages
assistant_message_effect = "FadeIn"  # AI responses  
system_message_effect = "SlideInUp"  # System notifications
error_message_effect = "Pulse"       # Error messages
```

### Available Animation Effects

| Effect | Description | Visual Behavior |
|--------|-------------|-----------------|
| `None` | No animation | Instant appearance |
| `FadeIn` | Smooth reveal | Content gradually becomes visible |
| `SlideInLeft` | Slide from left | Content moves rightward into view |
| `SlideInRight` | Slide from right | Content moves leftward into view |
| `SlideInUp` | Slide from bottom | Content moves upward into position |
| `SlideInDown` | Slide from top | Content moves downward into position |
| `Typewriter` | Character reveal | Characters appear one by one |
| `Pulse` | Placeholder effect | Intended for future styling |
| `Highlight` | Placeholder effect | Intended for future highlighting |

### Easing Functions

| Function | Behavior | Use Case |
|----------|----------|----------|
| `Linear` | Constant speed | Simple, predictable animations |
| `EaseIn` | Starts slow, ends fast | Building anticipation |
| `EaseOut` | Starts fast, ends slow | Natural deceleration |
| `EaseInOut` | Slow start, fast middle, slow end | Most natural feeling |
| `Bounce` | Bouncy overshoot | Playful, energetic effects |

## Using Animations in Code

### Basic Usage

1. **Access the animation state** in your component:
```rust
use crate::app::animation::{AnimationState, AnimationUtils, AnimationFactory};

fn render_message(
    frame: &mut Frame,
    message: &Message,
    animation_state: &AnimationState,
) {
    let message_id = message.id();
    
    // Check if message has an animation
    if animation_state.has_animation(&message_id) {
        let progress = animation_state.get_progress(&message_id).unwrap_or(1.0);
        let animation_type = animation_state.get_animation_type(&message_id).unwrap();
        
        // Apply the appropriate effect
        let animated_content = match animation_type {
            AnimationType::FadeIn { .. } => {
                AnimationUtils::apply_fade_effect(message.content(), progress)
            }
            AnimationType::SlideIn { direction, .. } => {
                AnimationUtils::apply_slide_effect(message.content(), progress, *direction)
            }
            AnimationType::Typewriter { .. } => {
                AnimationUtils::apply_typewriter_effect(message.content(), progress)
            }
            _ => message.content().to_string(),
        };
        
        // Render the animated content
        render_content(frame, &animated_content, /* ... */);
    } else {
        // No animation - render normally
        render_content(frame, message.content(), /* ... */);
    }
}
```

### Starting Animations

Animations are typically started in response to events:

```rust
use crate::{AppEvent, MessageUpdatedEvent, MessageKind};
use crate::app::animation::{AnimationFactory, AnimationType};

// In an event handler
async fn handle_message_updated(app: &mut App, event: MessageUpdatedEvent) {
    let animation_config = &app.state.config.read().await.animation;
    
    // Determine animation type based on message kind and user config
    let animation_type = match event.kind {
        MessageKind::User => match animation_config.new_message_effect {
            AnimationEffect::FadeIn => AnimationFactory::fade_in(Duration::from_millis(
                animation_config.default_duration_ms
            )),
            AnimationEffect::SlideInLeft => AnimationFactory::slide_in_left(Duration::from_millis(
                animation_config.default_duration_ms
            )),
            // ... other effects
            _ => return, // None effect
        },
        MessageKind::Assistant => /* ... */,
        MessageKind::System => /* ... */,
    };
    
    // Start the animation
    app.animation_state.start_animation(event.message_id, animation_type);
}
```

## Creating Custom Animations

### Step 1: Define Animation Type

Add your new animation to the `AnimationType` enum in `crates/ploke-tui/src/app/animation/mod.rs`:

```rust
#[derive(Debug, Clone)]
pub enum AnimationType {
    FadeIn {
        duration: Duration,
    },
    SlideIn {
        direction: SlideDirection,
        duration: Duration,
    },
    Typewriter {
        char_delay: Duration,
    },
    // Add your custom animation
    CustomWiggle {
        duration: Duration,
        intensity: f32, // 0.0 to 1.0
    },
    // ... existing types
}
```

### Step 2: Add Animation Utility Method

Implement the effect logic in `AnimationUtils`:

```rust
impl AnimationUtils {
    // ... existing methods
    
    /// Apply a custom wiggle effect to text content
    pub fn apply_wiggle_effect(content: &str, progress: f32, intensity: f32) -> String {
        if progress >= 1.0 {
            return content.to_string();
        }
        
        // Your custom animation logic here
        // This example adds visual markers to show the effect
        let total_chars = content.len();
        let visible_chars = (total_chars as f32 * progress) as usize;
        
        if visible_chars >= total_chars {
            format!("~{}~", content) // Wrapped in tildes
        } else {
            let visible_content: String = content.chars().take(visible_chars).collect();
            format!("~{}~", visible_content)
        }
    }
}
```

### Step 3: Add Factory Method

Create a convenient factory method:

```rust
impl AnimationFactory {
    // ... existing methods
    
    /// Create a custom wiggle animation
    pub fn wiggle(duration: Duration, intensity: f32) -> AnimationType {
        AnimationType::CustomWiggle {
            duration,
            intensity,
        }
    }
    
    /// Create a custom wiggle animation with default intensity
    pub fn wiggle_default() -> AnimationType {
        Self::wiggle(Duration::from_millis(500), 0.7)
    }
}
```

### Step 4: Update Duration Calculation

Add duration handling in `AnimationState::get_animation_duration()`:

```rust
fn get_animation_duration(&self, animation_type: &AnimationType) -> Duration {
    match animation_type {
        AnimationType::FadeIn { duration } => *duration,
        AnimationType::SlideIn { duration, .. } => *duration,
        AnimationType::Typewriter { .. } => self.config.default_duration,
        AnimationType::CustomWiggle { duration, .. } => *duration,
        // ... other cases
    }
}
```

### Step 5: Add Configuration Support

Update the user configuration to support your new effect:

```rust
// In crates/ploke-tui/src/user_config.rs
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub enum AnimationEffect {
    #[default]
    None,
    FadeIn,
    SlideInLeft,
    SlideInRight,
    SlideInUp,
    SlideInDown,
    Typewriter,
    Pulse,
    Highlight,
    // Add your new effect
    CustomWiggle,
}
```

### Step 6: Update Event Handler

Add support in the animation trigger handler:

```rust
// In crates/ploke-tui/src/app/events.rs
let animation_type = match message_event.kind {
    MessageKind::User => match animation_config.new_message_effect {
        AnimationEffect::None => return,
        AnimationEffect::FadeIn => AnimationFactory::fade_in(Duration::from_millis(
            animation_config.default_duration_ms,
        )),
        // ... existing effects
        AnimationEffect::CustomWiggle => AnimationFactory::wiggle(Duration::from_millis(
            animation_config.default_duration_ms,
        ), 0.7),
    },
    // ... other message kinds
};
```

### Step 7: Add UI Rendering Support

Update the rendering code to handle your new animation:

```rust
// In crates/ploke-tui/src/app/message_item.rs
let animated_content = match animation_type {
    AnimationType::FadeIn { .. } => {
        AnimationUtils::apply_fade_effect(msg.content(), progress)
    }
    AnimationType::SlideIn { direction, .. } => {
        AnimationUtils::apply_slide_effect(msg.content(), progress, *direction)
    }
    AnimationType::Typewriter { .. } => {
        AnimationUtils::apply_typewriter_effect(msg.content(), progress)
    }
    // Add your new animation
    AnimationType::CustomWiggle { intensity, .. } => {
        AnimationUtils::apply_wiggle_effect(msg.content(), progress, *intensity)
    }
    // ... other cases
};
```

## Advanced Usage

### Animation State Management

The animation system provides several methods for managing animation state:

```rust
// Check if an animation exists
if animation_state.has_animation(&message_id) {
    // Animation is active or completed
}

// Get current progress (0.0 to 1.0)
let progress = animation_state.get_progress(&message_id); // Option<f32>

// Get animation type
let animation_type = animation_state.get_animation_type(&message_id); // Option<&AnimationType>

// Update all animations (call this in your animation loop)
let completed_animations = animation_state.update_animations();

// Stop all animations
animation_state.stop_all_animations();

// Clean up old completed animations
animation_state.cleanup_old_animations(Duration::from_secs(30));
```

### Animation Loop Integration

Integrate animations into your application's main loop:

```rust
// In your main application loop
loop {
    // ... other event handling
    
    // Animation updates for smooth animations
    _ = &mut animation_tick => {
        let completed_animations = self.animation_state.update_animations();
        if !completed_animations.is_empty() {
            self.needs_redraw = true; // Trigger UI redraw
        }
        animation_tick.as_mut().reset(TokioInstant::now() + Duration::from_millis(16));
    }
}
```

### Custom Easing Functions

You can add custom easing functions by extending the `EasingFunction` enum:

```rust
// In crates/ploke-tui/src/user_config.rs
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub enum EasingFunction {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    // Add custom easing
    Elastic,
    Back,
}
```

Then implement the logic in `AnimationState::apply_easing()`:

```rust
fn apply_easing(&self, progress: f32) -> f32 {
    match self.config.easing_function {
        EasingFunction::Linear => progress,
        EasingFunction::EaseIn => progress * progress,
        EasingFunction::EaseOut => 1.0 - (1.0 - progress).powi(2),
        EasingFunction::EaseInOut => {
            if progress < 0.5 {
                2.0 * progress * progress
            } else {
                1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
            }
        }
        EasingFunction::Bounce => {
            // Existing bounce implementation
        }
        // Add custom easing
        EasingFunction::Elastic => {
            // Custom elastic easing logic
            if progress == 0.0 || progress == 1.0 {
                progress
            } else {
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                if progress == 0.0 {
                    0.0
                } else if progress == 1.0 {
                    1.0
                } else {
                    -(2.0f32).powf(10.0 * progress - 10.0) * ((progress * 10.0 - 11.125) * c4).sin() + 1.0
                }
            }
        }
    }
}
```

## Best Practices

### Performance

1. **Limit concurrent animations**: Too many simultaneous animations can impact performance
2. **Use appropriate durations**: Shorter durations (200-500ms) feel more responsive
3. **Clean up old animations**: Call `cleanup_old_animations()` periodically
4. **Avoid complex effects in tight loops**: Keep animation logic simple and efficient

### User Experience

1. **Respect user preferences**: Check `animation_config.enabled` before starting animations
2. **Provide meaningful durations**: Match animation speed to content importance
3. **Use consistent easing**: Stick to `EaseInOut` for most natural feel
4. **Test with different content lengths**: Ensure animations work well with short and long text

### Code Organization

1. **Keep animation logic in `AnimationUtils`**: Centralize effect implementations
2. **Use factory methods**: Provide convenient ways to create common animations
3. **Document custom animations**: Explain the visual behavior and intended use
4. **Test thoroughly**: Add unit tests for new animation effects

## Troubleshooting

### Common Issues

**Animation not appearing:**
- Check if `enabled = true` in config
- Verify `animation_state.has_animation()` returns true
- Ensure `update_animations()` is called regularly

**Janky or inconsistent animations:**
- Check animation loop timing (should be ~60fps)
- Verify duration calculations are correct
- Ensure easing functions are properly implemented

**Memory leaks:**
- Call `cleanup_old_animations()` periodically
- Check that completed animations aren't accumulating

**Animation ends too quickly:**
- Verify duration is being passed correctly from config
- Check for timing issues in `get_animation_duration()`

### Debug Tips

1. **Enable debug logging**: Add println! statements in animation methods
2. **Monitor animation state**: Log `has_animation()` and `get_progress()` results
3. **Check configuration**: Verify user config is being loaded correctly
4. **Test in isolation**: Create unit tests for specific animation behaviors

## Examples

See the test files in `crates/ploke-tui/src/app/animation/mod.rs` for comprehensive examples of:
- Animation progression testing
- Timing consistency verification
- UI rendering integration
- State persistence after completion

These tests demonstrate best practices and provide working examples of all animation features.