import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Shield, Settings, Power, X } from "lucide-react";

interface AppConfig {
  obfuscation: {
    strength: number;
    max_latency_ms: number;
    dwell_bucket_ms: number;
    flight_bucket_ms: number;
    noise_stddev_ms: number;
  };
}

function App() {
  const [isRunning, setIsRunning] = useState(false);
  const [hasPermissions, setHasPermissions] = useState(true);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [view, setView] = useState<"status" | "config">("status");
  const [statusMsg, setStatusMsg] = useState("");

  const checkStatus = async () => {
    const status = await invoke<boolean>("get_status");
    setIsRunning(status);
    
    const perms = await invoke<boolean>("check_permissions");
    setHasPermissions(perms);
  };

  const loadConfig = async () => {
    try {
      const cfg = await invoke<AppConfig>("get_config");
      setConfig(cfg);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    checkStatus();
    loadConfig();
    const interval = setInterval(checkStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  const toggleEngine = async () => {
    try {
      setStatusMsg("");
      if (isRunning) {
        await invoke("stop_engine");
      } else {
        const result = await invoke<string>("start_engine");
        console.log(result);
      }
      await checkStatus();
    } catch (e: any) {
      console.error(e);
      setStatusMsg(e.toString());
      setIsRunning(false);
    }
  };

  const saveConfig = async () => {
    if (!config) return;
    try {
      await invoke("save_config", { config });
      setStatusMsg("Config saved!");
      setTimeout(() => {
        setStatusMsg("");
        setView("status");
      }, 1000);
    } catch (e: any) {
      setStatusMsg(e.toString());
    }
  };

  return (
    <div className="flex flex-col h-screen p-6 select-none bg-zinc-950 text-zinc-100">
      <div className="flex items-center justify-between mb-8">
        <div className="flex items-center gap-2">
          <div className={`p-2 rounded-xl transition-colors duration-500 ${isRunning ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>
            <Shield size={24} />
          </div>
          <h1 className="text-xl font-bold tracking-tight">ShuffleKeys</h1>
        </div>
        <button 
          onClick={() => setView(view === 'status' ? 'config' : 'status')}
          className="p-2 hover:bg-zinc-800 rounded-lg transition-colors text-zinc-400"
        >
          {view === 'status' ? <Settings size={20} /> : <X size={20} />}
        </button>
      </div>

      <div className="flex-1 flex flex-col items-center justify-center">
        {view === 'status' ? (
          <div className="text-center flex flex-col items-center">
            <button
              onClick={toggleEngine}
              className={`w-40 h-40 rounded-full flex items-center justify-center mb-8 transition-all duration-500 outline-none
                ${isRunning 
                  ? 'bg-green-500 shadow-[0_0_50px_rgba(34,197,94,0.4)] scale-105 active:scale-95' 
                  : 'bg-zinc-800 hover:bg-zinc-700 active:scale-95'
                }`}
            >
              <Power 
                size={64} 
                className={`transition-colors duration-500 ${isRunning ? 'text-white' : 'text-zinc-500'}`} 
              />
            </button>
            <h2 className="text-2xl font-semibold mb-2">
              {isRunning ? 'Obfuscation Active' : 'System Paused'}
            </h2>
            {!hasPermissions && (
              <div className="mt-2 p-3 bg-red-500/10 border border-red-500/20 rounded-xl max-w-xs">
                <p className="text-red-400 text-xs font-bold leading-tight">
                  ACCESSIBILITY PERMISSIONS MISSING<br/>
                  <span className="font-normal opacity-80 mt-1 block">
                    Enable ShuffleKeys in System Settings -&gt; Privacy -&gt; Accessibility
                  </span>
                </p>
              </div>
            )}
            <p className="text-zinc-500 text-sm mt-2">
              {isRunning ? 'Defeating biometric fingerprinting' : 'Protection is currently disabled'}
            </p>
          </div>
        ) : (
          <div className="w-full space-y-6 overflow-y-auto max-h-[380px] pr-2 custom-scrollbar">
            <div className="space-y-2">
              <label className="text-xs uppercase font-bold text-zinc-500 tracking-wider">Obfuscation Strength</label>
              <input 
                type="range" min="0" max="1" step="0.1" 
                value={config?.obfuscation.strength || 0}
                onChange={(e) => setConfig(prev => prev ? {...prev, obfuscation: {...prev.obfuscation, strength: parseFloat(e.target.value)}} : null)}
                className="w-full h-1.5 bg-zinc-800 rounded-lg appearance-none cursor-pointer accent-white"
              />
              <div className="flex justify-between text-[10px] text-zinc-500 font-medium">
                <span>NATURAL</span>
                <span>MAXIMUM</span>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1">
                <label className="text-xs font-bold text-zinc-500">Max Latency (ms)</label>
                <input 
                  type="number" 
                  value={config?.obfuscation.max_latency_ms}
                  onChange={(e) => setConfig(prev => prev ? {...prev, obfuscation: {...prev.obfuscation, max_latency_ms: parseInt(e.target.value)}} : null)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm focus:border-zinc-600 outline-none transition-colors"
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-bold text-zinc-500">Noise σ (ms)</label>
                <input 
                  type="number"
                  value={config?.obfuscation.noise_stddev_ms}
                  onChange={(e) => setConfig(prev => prev ? {...prev, obfuscation: {...prev.obfuscation, noise_stddev_ms: parseInt(e.target.value)}} : null)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm focus:border-zinc-600 outline-none transition-colors"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1">
                <label className="text-xs font-bold text-zinc-500">Dwell Bucket (ms)</label>
                <input 
                  type="number" 
                  value={config?.obfuscation.dwell_bucket_ms}
                  onChange={(e) => setConfig(prev => prev ? {...prev, obfuscation: {...prev.obfuscation, dwell_bucket_ms: parseInt(e.target.value)}} : null)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm focus:border-zinc-600 outline-none transition-colors"
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-bold text-zinc-500">Flight Bucket (ms)</label>
                <input 
                  type="number"
                  value={config?.obfuscation.flight_bucket_ms}
                  onChange={(e) => setConfig(prev => prev ? {...prev, obfuscation: {...prev.obfuscation, flight_bucket_ms: parseInt(e.target.value)}} : null)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm focus:border-zinc-600 outline-none transition-colors"
                />
              </div>
            </div>

            <button 
              onClick={saveConfig}
              className="w-full bg-zinc-100 hover:bg-white text-black py-3 rounded-xl font-bold transition-all active:scale-95 mt-4"
            >
              Save Configuration
            </button>
          </div>
        )}
      </div>

      <div className="mt-auto h-8 flex items-center justify-center">
        <p className="text-xs text-zinc-600 font-medium">{statusMsg}</p>
      </div>

      <style>{`
        .custom-scrollbar::-webkit-scrollbar { width: 4px; }
        .custom-scrollbar::-webkit-scrollbar-track { background: transparent; }
        .custom-scrollbar::-webkit-scrollbar-thumb { background: #27272a; border-radius: 2px; }
        input[type='number']::-webkit-inner-spin-button, 
        input[type='number']::-webkit-outer-spin-button { 
          -webkit-appearance: none; 
          margin: 0; 
        }
      `}</style>
    </div>
  );
}

export default App;
