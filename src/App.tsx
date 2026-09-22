import { Dashboard } from "./components/Dashboard";
import { useTheme } from "./hooks/useTheme";

export default function App() {
  const { theme, toggleTheme } = useTheme();

  return (
    <div className={theme === "dark" ? "dark" : ""}>
      <div
        className={`min-h-screen antialiased ${
          theme === "dark"
            ? "bg-neutral-950 text-neutral-100"
            : "bg-neutral-50 text-neutral-900"
        }`}
      >
        <Dashboard theme={theme} toggleTheme={toggleTheme} />
      </div>
    </div>
  );
}
