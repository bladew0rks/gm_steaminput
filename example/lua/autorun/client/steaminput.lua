if not util.IsBinaryModuleInstalled("steaminput") then return end

local ok, err = pcall(require, "steaminput")
if not ok then
	ErrorNoHalt("[steaminput] failed to load module: " .. tostring(err) .. "\n")
	return
end

include("steaminput/base.lua")
