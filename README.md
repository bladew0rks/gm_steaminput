# gm_steaminput

clientside garry's mod binary module that gives lua access to steam's input api

## requirements
- x86-64 gmod
- rust nightly with the x86_64-pc-windows-gnu target, plus MinGW (x86_64-w64-mingw32-gcc)
- steam input enabled in steam settings

## building
```sh
make            # build release, install DLL + link addon
make debug      # same, debug build
make uninstall  # remove DLL and addon link
make GMOD_DIR=/path/to/GarrysMod   # custom install location
```

the binary module will be installed under garrysmod/lua/bin/gmcl_steaminput_win64.dll, addons will be symlinked to garrysmod/addons/steaminput

## lua api

```lua
require("steaminput")
```

**controllers**
| Function | Returns |
|---|---|
| `steaminput.GetConnectedControllers()` | list of controller handles (strings) |
| `steaminput.GetInputType(h)` | type name, type id (e.g. `"XBoxOneController", 3`) |
| `steaminput.TriggerVibration(h, left, right, seconds)` | rumble, 0–65535 per side, stops by itself after `seconds` if given |
| `steaminput.SetLEDColor(h, r, g, b)` / `ResetLEDColor(h)` | LED colour, on controllers that have one |
| `steaminput.ShowBindingPanel(h)` | opens Steam's binding screen |

**actions**
```lua
steaminput.RegisterActionSet("mysweps", {
    title = "My SWEPs",
    buttons  = { firemode = { title = "Fire Mode", default = "dpad_left" } },
    triggers = { aim = { title = "Aim", default = "lt" } },
    analog   = { lean = { title = "Lean", mode = "joystick_move", default = "right_stick" } },
})

steaminput.ActivateActionSet(h, "mysweps")                          -- h or "all"
local down, active = steaminput.GetDigitalAction(h, "mysweps", "firemode")
local x, y, active = steaminput.GetAnalogAction(h, "mysweps", "lean")
```

analog modes are `joystick_move` (stick position, -1 to 1) and `absolute_mouse` (mouse deltas)

defaults can be `a b x y`, `dpad_up/down/left/right`, `lb rb start back`, `lt rt`, `ls rs`, `left_stick right_stick` and `button_back_left/right[_upper]`

all registered sets get merged into one manifest with default layouts for every controller type, written to garrysmod/data/steaminput/ and loaded on the next frame. `steaminput.ApplyActions()` does it right away

**glyphs**
```lua
for _, origin in ipairs(steaminput.GetDigitalActionOrigins(h, "mysweps", "firemode")) do
    local mat = Material(steaminput.GetGlyph(origin))   -- size 0-2, style flags optional
    local name = steaminput.GetOriginName(origin)       -- e.g. "D-Pad Left"
end
```

## base addon

addon/lua/autorun/client/steaminput_base.lua registers an "On Foot" set and moves the player with it

| Input | Action |
|---|---|
| left stick | move |
| right stick | look |
| rt / lt | attack / attack2 |
| a | jump |
| b | crouch |
| x | reload |
| y | use |
| ls / rs | sprint / walk |
| lb / rb | prev / next weapon |
| dpad up | flashlight |
| back | scoreboard |
| start | menu |

settings are in the spawnmenu under utilities > steam input, or the convars `steaminput_enabled`, `steaminput_overlay`, `steaminput_look_sensitivity` and `steaminput_look_invert`. `steaminput_bindings` opens steam's binding screen
