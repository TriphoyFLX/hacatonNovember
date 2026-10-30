import { Navigate, Route, Routes } from 'react-router-dom';
import { AuthBootstrap } from './components/auth/AuthBootstrap';
import { RequireAuth } from './components/auth/RequireAuth';
import { AppShell } from './components/layout/AppShell';
import { modes } from './data/modes';
import { AirInstrumentsPage } from './pages/AirInstrumentsPage';
import { HomePage } from './pages/HomePage';
import { LoginPage } from './pages/LoginPage';
import { ModePlaceholderPage } from './pages/ModePlaceholderPage';
import { RegisterPage } from './pages/RegisterPage';
import { SettingsPage } from './pages/SettingsPage';

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
            {modes
              .filter((mode) => mode.id !== 'air-instruments')
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
