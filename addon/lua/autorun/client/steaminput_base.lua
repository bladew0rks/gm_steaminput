if not util.IsBinaryModuleInstalled("steaminput") then return end

local ok, err = pcall(require, "steaminput")
if not ok then
	ErrorNoHalt("[steaminput] failed to load module: " .. tostring(err) .. "\n")
	return
end

local SET = "base"

local BUTTON_ORDER = {
	"attack", "attack2", "jump", "duck", "reload", "use", "sprint",
	"walk", "invprev", "invnext", "flashlight", "scores", "menu",
}

local BUTTONS = {
	attack = { title = "Primary Attack", default = "rt", key = IN_ATTACK, short = "Attack" },
	attack2 = { title = "Secondary Attack", default = "lt", key = IN_ATTACK2, short = "Attack 2" },
	jump = { title = "Jump", default = "a", key = IN_JUMP, short = "Jump" },
	duck = { title = "Crouch", default = "b", key = IN_DUCK, short = "Crouch" },
	reload = { title = "Reload", default = "x", key = IN_RELOAD, short = "Reload" },
	use = { title = "Use", default = "y", key = IN_USE, short = "Use" },
	sprint = { title = "Sprint", default = "ls", key = IN_SPEED, short = "Sprint" },
	walk = { title = "Walk", default = "rs", key = IN_WALK, short = "Walk" },
	invprev = { title = "Previous Weapon", default = "lb", short = "Prev Wep" },
	invnext = { title = "Next Weapon", default = "rb", short = "Next Wep" },
	flashlight = { title = "Flashlight", default = "dpad_up", short = "Light" },
	scores = { title = "Scoreboard", default = "back", short = "Scores" },
	menu = { title = "Menu", default = "start", short = "Menu" },
}

steaminput.RegisterActionSet(SET, {
	title = "On Foot",
	analog = {
		move = { title = "Move", mode = "joystick_move", default = "left_stick" },
		look = { title = "Look", mode = "absolute_mouse", default = "right_stick" },
	},
	buttons = BUTTONS,
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

local nextScan = 0
local scoresShown = false
local lastLookFrame = -1

local function scanController()
	local handle = steaminput.GetConnectedControllers()[1]
	state.controller = handle
	state.typeName = handle and (steaminput.GetInputType(handle)) or nil
end

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

hook.Add("Think", "steaminput_base", function()
	if CurTime() >= nextScan then
		nextScan = CurTime() + 0.5
		scanController()
	end

	local h = state.controller
	if not h then return end

	steaminput.ActivateActionSet(h, SET)

	for _, name in ipairs(BUTTON_ORDER) do
		local down = steaminput.GetDigitalAction(h, SET, name)
		if down and not state.held[name] then state.queued[name] = true end
		state.held[name] = down
	end

	state.move.x, state.move.y = steaminput.GetAnalogAction(h, SET, "move")
	state.look.x, state.look.y = steaminput.GetAnalogAction(h, SET, "look")

	if not cv_enabled:GetBool() then
		state.queued = {}
		return
	end

	if state.queued.menu then
		state.queued.menu = nil
		if gui.IsGameUIVisible() then gui.HideGameUI() else gui.ActivateGameUI() end
	end

	if state.held.scores ~= scoresShown then
		scoresShown = state.held.scores
		RunConsoleCommand(scoresShown and "+showscores" or "-showscores")
	end
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

	for name, def in pairs(BUTTONS) do
		if def.key and held[name] then cmd:AddKey(def.key) end
	end

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
surface.CreateFont("SteamInputOverlaySmall", { font = "Roboto", size = 12, weight = 500 })

local COLOR_BG = Color(15, 17, 22, 200)
local COLOR_RING = Color(255, 255, 255, 40)
local COLOR_DOT = Color(102, 192, 244)
local COLOR_CHIP = Color(255, 255, 255, 18)
local COLOR_CHIP_ON = Color(102, 192, 244, 220)
local COLOR_TEXT = Color(230, 232, 236)
local COLOR_MUTED = Color(150, 155, 165)

local function drawStick(cx, cy, r, x, y, label)
	surface.DrawCircle(cx, cy, r, COLOR_RING)
	surface.DrawCircle(cx, cy, 1, COLOR_RING)
	local dx, dy = cx + math.Clamp(x, -1, 1) * r, cy - math.Clamp(y, -1, 1) * r
	draw.RoundedBox(4, dx - 4, dy - 4, 8, 8, COLOR_DOT)
	draw.SimpleText(label, "SteamInputOverlaySmall", cx, cy + r + 8, COLOR_MUTED, TEXT_ALIGN_CENTER, TEXT_ALIGN_TOP)
end

hook.Add("HUDPaint", "steaminput_overlay", function()
	if not cv_overlay:GetBool() then return end

	local chipW, chipH, gap, cols = 70, 20, 4, 4
	local rows = math.ceil(#BUTTON_ORDER / cols)
	local w = 2 * 12 + 2 * 90 + cols * chipW + (cols - 1) * gap + 16
	local h = math.max(24 + 100, 30 + rows * (chipH + gap)) + 12
	local x, y = 24, ScrH() - h - 24

	draw.RoundedBox(8, x, y, w, h, COLOR_BG)

	if not state.controller then
		draw.SimpleText("No Steam Input controller", "SteamInputOverlay", x + w / 2, y + h / 2, COLOR_MUTED, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
		return
	end

	draw.SimpleText(state.typeName or "Controller", "SteamInputOverlay", x + 12, y + 8, COLOR_TEXT)
	if not cv_enabled:GetBool() then
		draw.SimpleText("disabled", "SteamInputOverlaySmall", x + w - 12, y + 10, COLOR_MUTED, TEXT_ALIGN_RIGHT)
	end

	local stickY = y + 30 + 36
	drawStick(x + 12 + 45, stickY, 32, state.move.x, state.move.y, "Move")
	drawStick(x + 12 + 135, stickY, 32, state.look.x / 20, -state.look.y / 20, "Look")

	local chipX0 = x + 12 + 2 * 90 + 16
	for i, name in ipairs(BUTTON_ORDER) do
		local col, row = (i - 1) % cols, math.floor((i - 1) / cols)
		local cx, cy = chipX0 + col * (chipW + gap), y + 30 + row * (chipH + gap)
		local on = state.held[name]
		draw.RoundedBox(4, cx, cy, chipW, chipH, on and COLOR_CHIP_ON or COLOR_CHIP)
		draw.SimpleText(BUTTONS[name].short, "SteamInputOverlaySmall", cx + chipW / 2, cy + chipH / 2, on and color_black or COLOR_TEXT, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
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
