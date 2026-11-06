local MOUSE_THRESHOLD = 2
local CURSOR_THRESHOLD = 4
local STICK_THRESHOLD = 0.25
local LOOK_THRESHOLD = 1
local GLYPH_CACHE_SECONDS = 1

local mode = "keyboard"
local keysDown = {}
local lastCursorX, lastCursorY = 0, 0
local actionKinds = {}
local actionKindsExpire = 0

local function setMode(new)
	if mode == new then return end
	mode = new
	hook.Run("SteamInputModeChanged", new)
end

function steaminput.GetInputMode()
	return mode
end

function steaminput.IsUsingController()
	return mode == "controller"
end

local function kindsOf(set)
	if RealTime() > actionKindsExpire then
		actionKinds, actionKindsExpire = {}, RealTime() + 5
	end
	local kinds = actionKinds[set]
	if kinds == nil then
		kinds = steaminput.GetActions(set) or false
		actionKinds[set] = kinds
	end
	return kinds
end

local function analogActive(h, set)
	local kinds = kindsOf(set)
	if not kinds then return false end
	for action, kind in pairs(kinds) do
		if kind ~= "button" then
			local x, y = steaminput.GetAnalogAction(h, set, action)
			local threshold = kind == "mouse" and LOOK_THRESHOLD or STICK_THRESHOLD
			if math.abs(x) > threshold or math.abs(y) > threshold then return true end
		end
	end
	return false
end

local function keyboardPressed()
	local pressed = false
	for code = KEY_FIRST + 1, MOUSE_LAST do
		local down = input.IsButtonDown(code)
		if down and not keysDown[code] then pressed = true end
		keysDown[code] = down or nil
	end
	return pressed
end

hook.Add("SteamInputActionPressed", "steaminput_mode", function()
	setMode("controller")
end)

hook.Add("Think", "steaminput_mode", function()
	if keyboardPressed() then
		setMode("keyboard")
		return
	end

	local cx, cy = input.GetCursorPos()
	if vgui.CursorVisible() and math.abs(cx - lastCursorX) + math.abs(cy - lastCursorY) > CURSOR_THRESHOLD then
		setMode("keyboard")
	end
	lastCursorX, lastCursorY = cx, cy

	if mode == "controller" then return end
	local h = steaminput.GetPrimaryController()
	if not h then return end
	local set = steaminput.GetCurrentActionSet(h)
	if set and analogActive(h, set) then
		setMode("controller")
		return
	end
	for _, layer in ipairs(steaminput.GetActiveActionSetLayers(h)) do
		if analogActive(h, layer) then
			setMode("controller")
			return
		end
	end
end)

hook.Add("CreateMove", "steaminput_mode", function(cmd)
	if math.abs(cmd:GetMouseX()) + math.abs(cmd:GetMouseY()) > MOUSE_THRESHOLD then
		setMode("keyboard")
	end
end)

local glyphs = {}
local materials = {}

function steaminput.GetActionGlyph(set, action, size, h)
	h = h or steaminput.GetPrimaryController()
	if not h then return end

	local key = h .. "/" .. set .. "/" .. action .. "/" .. (size or 0)
	local cached = glyphs[key]
	if cached and cached.expires > RealTime() then return cached.mat end

	local origins = steaminput.GetDigitalActionOrigins(h, set, action)
	if #origins == 0 then origins = steaminput.GetAnalogActionOrigins(h, set, action) end

	local mat
	local path = origins[1] and steaminput.GetGlyph(origins[1], size or 0)
	if path then
		mat = materials[path] or Material(path, "smooth mips")
		materials[path] = mat
	end

	glyphs[key] = { mat = mat, name = origins[1] and steaminput.GetOriginName(origins[1]), expires = RealTime() + GLYPH_CACHE_SECONDS }
	return mat
end

function steaminput.GetPromptText(set, action, bind, h)
	if mode == "controller" then
		h = h or steaminput.GetPrimaryController()
		local origins = h and steaminput.GetDigitalActionOrigins(h, set, action) or {}
		if #origins == 0 and h then origins = steaminput.GetAnalogActionOrigins(h, set, action) end
		if origins[1] then return steaminput.GetOriginName(origins[1]) end
	end
	local key = bind and input.LookupBinding(bind)
	return key and string.upper(key) or nil
end

surface.CreateFont("SteamInputPrompt", { font = "Roboto", size = 16, weight = 700 })

local COLOR_KEY_BG = Color(255, 255, 255, 30)

function steaminput.DrawPrompt(set, action, x, y, opts)
	opts = opts or {}
	local size = opts.size or 24
	local color = opts.color or color_white

	if mode == "controller" then
		local mat = steaminput.GetActionGlyph(set, action, opts.glyphSize)
		if mat and not mat:IsError() then
			surface.SetDrawColor(255, 255, 255, color.a)
			surface.SetMaterial(mat)
			surface.DrawTexturedRect(x, y, size, size)
			return size, size
		end
	end

	local text = steaminput.GetPromptText(set, action, opts.bind) or "?"
	local font = opts.font or "SteamInputPrompt"
	surface.SetFont(font)
	local tw = surface.GetTextSize(text)
	local w = math.max(size, tw + 10)
	draw.RoundedBox(0, x, y, w, size, COLOR_KEY_BG)
	draw.SimpleText(text, font, x + w / 2, y + size / 2, color, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
	return w, size
end
