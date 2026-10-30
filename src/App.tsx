import { Navigate, Route, Routes } from 'react-router-dom';
import { AppShell } from './components/layout/AppShell';
import { HomePage } from './pages/HomePage';
import { ModePlaceholderPage } from './pages/ModePlaceholderPage';
import { SettingsPage } from './pages/SettingsPage';
import { modes } from './data/modes';

export function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<HomePage />} />
        {modes.map((mode) => (
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
    </Routes>
  );
}
