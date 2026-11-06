local SET = "base"

local BUTTONS = {
	{ name = "attack", title = "Primary Attack", default = "rt", key = IN_ATTACK },
	{ name = "attack2", title = "Secondary Attack", default = "lt", key = IN_ATTACK2 },
	{ name = "jump", title = "Jump", default = "a", key = IN_JUMP },
	{ name = "duck", title = "Crouch", default = "b", key = IN_DUCK },
	{ name = "reload", title = "Reload", default = "x", key = IN_RELOAD },
	{ name = "use", title = "Use", default = "y", key = IN_USE },
	{ name = "sprint", title = "Sprint", default = "ls", key = IN_SPEED },
	{ name = "walk", title = "Walk", default = "rs", key = IN_WALK },
	{ name = "invprev", title = "Previous Weapon", default = "lb" },
	{ name = "invnext", title = "Next Weapon", default = "rb" },
	{ name = "flashlight", title = "Flashlight", default = "dpad_up" },
	{ name = "scores", title = "Scoreboard", default = "back" },
	{ name = "menu", title = "Menu", default = "start" },
}

local buttonDefs = {}
for _, b in ipairs(BUTTONS) do buttonDefs[b.name] = b end

steaminput.RegisterActionSet(SET, {
	title = "On Foot",
	analog = {
		move = { title = "Move", mode = "joystick_move", default = "left_stick" },
		look = { title = "Look", mode = "absolute_mouse", default = { "right_stick", "gyro" } },
	},
	buttons = buttonDefs,
})

local cv_enabled = CreateClientConVar("steaminput_enabled", "1", true, false, "Drive the player with a Steam Input controller")
local cv_sensitivity = CreateClientConVar("steaminput_look_sensitivity", "1", true, false, "Controller look sensitivity multiplier")
local cv_invert = CreateClientConVar("steaminput_look_invert", "0", true, false, "Invert controller look pitch")
local cv_overlay = CreateClientConVar("steaminput_overlay", "0", true, false, "Show the controller input overlay")

local state = {
	controller = nil,
	typeName = nil,
	held = {},
	queued = {},
	move = { x = 0, y = 0 },
	look = { x = 0, y = 0 },
}

local lastLookFrame = -1

local function cycleWeapon(ply, dir)
	local weapons = ply:GetWeapons()
	if #weapons < 2 then return end

	table.sort(weapons, function(a, b)
		if a:GetSlot() ~= b:GetSlot() then return a:GetSlot() < b:GetSlot() end
		return a:GetSlotPos() < b:GetSlotPos()
	end)

	local current = ply:GetActiveWeapon()
	for i, wep in ipairs(weapons) do
		if wep == current then
			return weapons[(i - 1 + dir) % #weapons + 1]
		end
	end
	return weapons[1]
end

local PRESS_ACTIONS = {
	menu = function()
		if gui.IsGameUIVisible() then gui.HideGameUI() else gui.ActivateGameUI() end
	end,
	scores = function() RunConsoleCommand("+showscores") end,
}

hook.Add("SteamInputActionPressed", "steaminput_base", function(h, set, action)
	if h ~= state.controller or set ~= SET then return end
	state.held[action] = true
	if not cv_enabled:GetBool() then return end
	if PRESS_ACTIONS[action] then PRESS_ACTIONS[action]() else state.queued[action] = true end
end)

hook.Add("SteamInputActionReleased", "steaminput_base", function(h, set, action)
	if h ~= state.controller or set ~= SET then return end
	state.held[action] = nil
	if action == "scores" then RunConsoleCommand("-showscores") end
end)

hook.Add("Think", "steaminput_base", function()
	local h = steaminput.GetPrimaryController()
	if h ~= state.controller then
		state.controller, state.typeName = h, h and (steaminput.GetInputType(h)) or nil
		state.held, state.queued = {}, {}
	end
	if not h then return end
	if steaminput.GetCurrentActionSet(h) ~= SET then steaminput.ActivateActionSet(h, SET) end
	state.move.x, state.move.y = steaminput.GetAnalogAction(h, SET, "move")
	state.look.x, state.look.y = steaminput.GetAnalogAction(h, SET, "look")
end)

hook.Add("CreateMove", "steaminput_base", function(cmd)
	if not state.controller or not cv_enabled:GetBool() then return end
	if gui.IsGameUIVisible() or vgui.CursorVisible() then return end
	local ply = LocalPlayer()
	if not IsValid(ply) then return end

	local held, queued = state.held, state.queued
	local mx, my = state.move.x, state.move.y
	if mx ~= 0 or my ~= 0 then
		local speed = held.walk and ply:GetSlowWalkSpeed() or held.sprint and ply:GetRunSpeed() or ply:GetWalkSpeed()
		cmd:SetForwardMove(math.Clamp(cmd:GetForwardMove() + my * speed, -speed, speed))
		cmd:SetSideMove(math.Clamp(cmd:GetSideMove() + mx * speed, -speed, speed))
	end

	if FrameNumber() ~= lastLookFrame then
		lastLookFrame = FrameNumber()
		local lx, ly = state.look.x, state.look.y
		if lx ~= 0 or ly ~= 0 then
			local scale = cv_sensitivity:GetFloat() * GetConVar("sensitivity"):GetFloat() * 0.022
			local pitchSign = cv_invert:GetBool() and -1 or 1
			local ang = cmd:GetViewAngles()
			ang.p = math.Clamp(ang.p + ly * scale * pitchSign, -89, 89)
			ang.y = ang.y - lx * scale
			cmd:SetViewAngles(ang)
		end
	end

	for _, b in ipairs(BUTTONS) do
		if b.key and held[b.name] then cmd:AddKey(b.key) end
	end

	if cmd:CommandNumber() == 0 then return end

	if queued.flashlight then
		queued.flashlight = nil
		cmd:SetImpulse(100)
	end

	for name, dir in pairs({ invprev = -1, invnext = 1 }) do
		if queued[name] then
			queued[name] = nil
			local wep = cycleWeapon(ply, dir)
			if IsValid(wep) then cmd:SelectWeapon(wep) end
		end
	end
end)

surface.CreateFont("SteamInputOverlay", { font = "Roboto", size = 15, weight = 600 })
surface.CreateFont("SteamInputOverlaySmall", { font = "Roboto", size = 13, weight = 500 })

local COLOR_BG = Color(15, 17, 22, 210)
local COLOR_RING = Color(255, 255, 255, 40)
local COLOR_DOT = Color(102, 192, 244)
local COLOR_ROW = Color(255, 255, 255, 12)
local COLOR_ROW_ON = Color(102, 192, 244, 200)
local COLOR_TEXT = Color(230, 232, 236)
local COLOR_MUTED = Color(150, 155, 165)

local ANALOG = {
	{ name = "move", label = "Move", scale = 1, flipY = false },
	{ name = "look", label = "Look", scale = 1 / 20, flipY = true },
}

local origins = {}
local nextOriginScan = 0
local originController
local materials = {}

local function glyphMaterial(origin)
	local path = steaminput.GetGlyph(origin, 0, 0)
	if not path then return end
	local mat = materials[path]
	if not mat then
		mat = Material(path, "smooth mips")
		materials[path] = mat
	end
	return mat
end

local function refreshOrigins()
	local h = state.controller
	origins = {}
	if not h then return end
	for _, b in ipairs(BUTTONS) do
		origins[b.name] = steaminput.GetDigitalActionOrigins(h, SET, b.name)
	end
	for _, a in ipairs(ANALOG) do
		origins[a.name] = steaminput.GetAnalogActionOrigins(h, SET, a.name)
	end
end

local function drawGlyphs(list, x, y, size)
	if not list or #list == 0 then
		draw.RoundedBox(0, x, y, size, size, COLOR_ROW)
		draw.SimpleText("?", "SteamInputOverlaySmall", x + size / 2, y + size / 2, COLOR_MUTED, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
		return size
	end
	local cx = x
	for i = 1, math.min(#list, 2) do
		local mat = glyphMaterial(list[i])
		if mat and not mat:IsError() then
			surface.SetDrawColor(255, 255, 255)
			surface.SetMaterial(mat)
			surface.DrawTexturedRect(cx, y, size, size)
		else
			local name = steaminput.GetOriginName(list[i]) or "?"
			draw.SimpleText(name, "SteamInputOverlaySmall", cx, y + size / 2, COLOR_TEXT, TEXT_ALIGN_LEFT, TEXT_ALIGN_CENTER)
		end
		cx = cx + size + 2
	end
	return cx - x - 2
end

local function drawStick(cx, cy, r, x, y)
	surface.DrawCircle(cx, cy, r, COLOR_RING)
	local dx, dy = cx + math.Clamp(x, -1, 1) * r, cy - math.Clamp(y, -1, 1) * r
	draw.RoundedBox(4, dx - 4, dy - 4, 8, 8, COLOR_DOT)
end

hook.Add("HUDPaint", "steaminput_overlay", function()
	if not cv_overlay:GetBool() then return end

	if state.controller ~= originController or CurTime() >= nextOriginScan then
		originController = state.controller
		nextOriginScan = CurTime() + 2
		refreshOrigins()
	end

	local pad, glyph, rowH, colW = 10, 22, 26, 150
	local cols = 2
	local rows = math.ceil(#BUTTONS / cols)
	local w = pad * 2 + cols * colW + (cols - 1) * 6
	local stickH = 86
	local h = pad + 20 + stickH + rows * rowH + pad
	local x, y = 24, 24

	if not state.controller then
		draw.RoundedBox(0, x, y, w, 40, COLOR_BG)
		draw.SimpleText("No Steam Input controller", "SteamInputOverlay", x + w / 2, y + 20, COLOR_MUTED, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
		return
	end

	draw.RoundedBox(0, x, y, w, h, COLOR_BG)
	local header = (state.typeName or "Controller") .. (cv_enabled:GetBool() and "" or " (disabled)")
	draw.SimpleText(header, "SteamInputOverlay", x + pad, y + pad, COLOR_TEXT)

	local gyro = steaminput.GetMotionData(state.controller).gyro
	if not gyro:IsZero() then
		local text = string.format("gyro %4d %4d %4d", gyro.x, gyro.y, gyro.z)
		draw.SimpleText(text, "SteamInputOverlaySmall", x + w - pad, y + pad + 1, COLOR_MUTED, TEXT_ALIGN_RIGHT)
	end

	local sy = y + pad + 22
	for i, a in ipairs(ANALOG) do
		local colX = x + pad + (i - 1) * (colW + 6)
		local v = state[a.name]
		drawGlyphs(origins[a.name], colX, sy, glyph)
		draw.SimpleText(a.label, "SteamInputOverlaySmall", colX + glyph + 6, sy + glyph / 2, COLOR_TEXT, TEXT_ALIGN_LEFT, TEXT_ALIGN_CENTER)
		drawStick(colX + colW / 2, sy + glyph + 32, 26, v.x * a.scale, (a.flipY and -v.y or v.y) * a.scale)
	end

	local by = sy + stickH
	for i, b in ipairs(BUTTONS) do
		local col, row = (i - 1) % cols, math.floor((i - 1) / cols)
		local rx, ry = x + pad + col * (colW + 6), by + row * rowH
		local on = state.held[b.name]
		draw.RoundedBox(0, rx, ry, colW, rowH - 4, on and COLOR_ROW_ON or COLOR_ROW)
		local gw = drawGlyphs(origins[b.name], rx + 2, ry + (rowH - 4 - glyph) / 2, glyph)
		draw.SimpleText(b.title, "SteamInputOverlaySmall", rx + gw + 8, ry + (rowH - 4) / 2, on and color_black or COLOR_TEXT, TEXT_ALIGN_LEFT, TEXT_ALIGN_CENTER)
	end
end)

local function openBindingPanel()
	if state.controller then
		steaminput.ShowBindingPanel(state.controller)
	else
		chat.AddText(COLOR_DOT, "[steaminput] ", COLOR_TEXT, "No Steam Input controller connected.")
	end
end

concommand.Add("steaminput_bindings", openBindingPanel, nil, "Open Steam's binding panel for the connected controller")

hook.Add("PopulateToolMenu", "steaminput_options", function()
	spawnmenu.AddToolMenuOption("Utilities", "Steam Input", "steaminput_options", "Controller", "", "", function(panel)
		panel:ClearControls()

		local status = panel:Help("")
		status.Think = function(self)
			local text = state.controller and ("Connected: " .. (state.typeName or "Controller")) or "No Steam Input controller connected"
			if self:GetText() ~= text then self:SetText(text) end
		end

		panel:CheckBox("Enable controller", "steaminput_enabled")
		panel:CheckBox("Show input overlay", "steaminput_overlay")
		panel:NumSlider("Look sensitivity", "steaminput_look_sensitivity", 0.1, 5, 2)
		panel:CheckBox("Invert look", "steaminput_look_invert")

		local bindings = panel:Button("Edit controller bindings")
		bindings.DoClick = openBindingPanel

		panel:ControlHelp("Opens Steam's binding screen. Steam only shows it while Garry's Mod has focus.")
	end)
end)
