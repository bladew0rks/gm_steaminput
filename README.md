# gm_steaminput

clientside garry's mod binary module that gives lua access to steam's input api

## requirements
- x86-64 gmod
- rust nightly with the x86_64-pc-windows-gnu target, plus MinGW (x86_64-w64-mingw32-gcc)
- steam input enabled in steam settings

## installing
put `gmcl_steaminput_win64.dll` from the latest release in garrysmod/lua/bin

## building
```sh
make            # build + install
make debug      # debug build
make uninstall
make GMOD_DIR=/path/to/GarrysMod
```

## lua api

```lua
require("steaminput")
```

**controllers**
```lua
steaminput.GetConnectedControllers()
steaminput.GetPrimaryController()
steaminput.GetInputType(h)                         -- "XBoxOneController", 3
steaminput.TriggerVibration(h, left, right, seconds)
steaminput.SetLEDColor(h, r, g, b)
steaminput.ResetLEDColor(h)
steaminput.ShowBindingPanel(h)
```

**actions**
```lua
steaminput.RegisterActionSet("mysweps", {
    title = "My SWEPs",
    buttons  = { firemode = { title = "Fire Mode", default = "dpad_left" } },
    triggers = { aim = { title = "Aim", default = "lt" } },
    analog   = { lean = { title = "Lean", mode = "joystick_move", default = "right_stick" } },
})

steaminput.ActivateActionSet(h, "mysweps")
steaminput.GetCurrentActionSet(h)
steaminput.GetDigitalAction(h, "mysweps", "firemode")   -- down, active
steaminput.GetAnalogAction(h, "mysweps", "lean")        -- x, y, active
steaminput.StopAnalogActionMomentum(h, "mysweps", "lean")
steaminput.GetActions("mysweps")

steaminput.IsDown("mysweps", "firemode", h)
steaminput.WasPressed("mysweps", "firemode", h)
steaminput.WasReleased("mysweps", "firemode", h)
```

analog modes: `joystick_move`, `absolute_mouse`

defaults: `a b x y`, `dpad_up/down/left/right`, `lb rb start back`, `lt rt`, `ls rs`, `left_stick right_stick`, `gyro`, `button_back_left/right[_upper]`

`h` is optional on IsDown/WasPressed/WasReleased

**layers**
```lua
steaminput.RegisterActionLayer("mylayer", "mysweps", {
    title = "My Layer",
    buttons = { inspect = { title = "Inspect", default = "y" } },
})

steaminput.ActivateActionSetLayer(h, "mylayer")
steaminput.DeactivateActionSetLayer(h, "mylayer")
steaminput.DeactivateAllActionSetLayers(h)
steaminput.GetActiveActionSetLayers(h)
```

**hooks**
```lua
SteamInputControllerConnected(h, typeName)
SteamInputControllerDisconnected(h)
SteamInputActionPressed(h, set, action)
SteamInputActionReleased(h, set, action)
SteamInputModeChanged(mode)
```

```lua
hook.Add("SteamInputActionPressed", "mysweps", function(h, set, action)
    if set == "mysweps" and action == "firemode" then
        LocalPlayer():GetActiveWeapon():CycleFireMode()
    end
end)
```

**haptics**
```lua
steaminput.Haptic(h, effect, strength, seconds, side)
steaminput.TriggerVibrationExtended(h, left, right, leftTrigger, rightTrigger, seconds)
steaminput.TriggerHapticPulse(h, side, intensity, gainDB)
steaminput.SetTriggerEffect(h, side, mode, a, b, c, seconds)
```

| Effect | Xbox | DualSense | Steam Controller/Deck | other |
|---|---|---|---|---|
| `rumble` | rumble | rumble | haptics | rumble |
| `trigger` | impulse triggers | adaptive triggers | rumble | rumble |
| `click` | rumble | rumble | haptic pulse | rumble |
| `resistance` | - | adaptive triggers | - | - |

```lua
local h = steaminput.GetPrimaryController()
steaminput.Haptic(h, "trigger", 0.6, 0.1, "right")
steaminput.Haptic(h, "rumble", 1, 0.3)
steaminput.TriggerVibration(h, 30000, 30000, 0.5)
```

**motion**
```lua
steaminput.GetMotionData(h)   -- { gyro = Vector, accel = Vector, orientation = { x, y, z, w } }
```

```lua
local gyro = steaminput.GetMotionData(h).gyro
local lean = math.Clamp(gyro.y / 200, -1, 1)
```

**input mode & prompts**
```lua
steaminput.IsUsingController()
steaminput.GetInputMode()   -- "controller" or "keyboard"
steaminput.DrawPrompt("mysweps", "firemode", x, y, { bind = "+reload", size = 24 })
steaminput.GetPromptText("mysweps", "firemode", "+reload")
steaminput.GetActionGlyph("mysweps", "firemode")
```

```lua
hook.Add("HUDPaint", "mysweps", function()
    local w = steaminput.DrawPrompt("mysweps", "firemode", 20, 20, { bind = "+zoom" })
    draw.SimpleText("fire mode", "DermaDefault", 20 + w + 6, 32, color_white, TEXT_ALIGN_LEFT, TEXT_ALIGN_CENTER)
end)
```

**glyphs**
```lua
steaminput.GetDigitalActionOrigins(h, set, action)
steaminput.GetAnalogActionOrigins(h, set, action)
steaminput.GetGlyph(origin, size, style)
steaminput.GetOriginName(origin)
```

```lua
for _, origin in ipairs(steaminput.GetDigitalActionOrigins(h, "mysweps", "firemode")) do
    local mat = Material(steaminput.GetGlyph(origin))
    local name = steaminput.GetOriginName(origin)
end
```

## example
example/ is an addon that drives the player with a controller

## license
MIT
