import { Navigate, Route, Routes } from 'react-router-dom';
import { AuthBootstrap } from './components/auth/AuthBootstrap';
import { RequireAuth } from './components/auth/RequireAuth';
import { AppShell } from './components/layout/AppShell';
import { modes } from './data/modes';
import { AirInstrumentsPage } from './pages/AirInstrumentsPage';
import { HomePage } from './pages/HomePage';
import { LoginPage } from './pages/LoginPage';
import { ModePlaceholderPage } from './pages/ModePlaceholderPage';
import { MagicHandsPage } from './pages/MagicHandsPage';
import { MotionArtPage } from './pages/MotionArtPage';
import { RegisterPage } from './pages/RegisterPage';
import { SettingsPage } from './pages/SettingsPage';

const IMPLEMENTED_MODE_IDS = new Set(['air-instruments', 'motion-art', 'magic-hands']);

export function App() {
  return (
    <AuthBootstrap>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/register" element={<RegisterPage />} />

        <Route element={<RequireAuth />}>
          <Route element={<AppShell />}>
            <Route index element={<HomePage />} />
            <Route path="/air-instruments" element={<AirInstrumentsPage />} />
            <Route path="/magic-hands" element={<MagicHandsPage />} />
            <Route path="/motion-art" element={<MotionArtPage />} />
            {modes
              .filter((mode) => !IMPLEMENTED_MODE_IDS.has(mode.id))
              .map((mode) => (
                <Route
                  key={mode.id}
                  path={mode.path}
                  element={
                    <ModePlaceholderPage
                      title={mode.title}
                      description={mode.description}
                      accent={mode.accent}
                    />
                  }
                />
              ))}
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Route>
      </Routes>
    </AuthBootstrap>
  );
}
