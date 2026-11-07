// Coquille de la SPA : en-tête, bandeaux, routes. Premier rendu immédiat depuis la référence en
// cache (AD-19) ; tout chemin inconnu affiche l'accueil.

import { useEffect } from "react";
import { BrowserRouter, Route, Routes } from "react-router";
import { useReference } from "./api/reference";
import { Banners } from "./components/Banner";
import { Header } from "./components/Header";
import { t, useLang } from "./i18n";
import { Home } from "./pages/Home";
import { EventPage } from "./pages/EventPage";
import { NewEvent } from "./pages/NewEvent";
import { OrganizerPage } from "./pages/OrganizerPage";
import { ParticipantPage } from "./pages/ParticipantPage";

export function App() {
  const lang = useLang();
  const reference = useReference();
  useEffect(() => {
    document.title = t("app.name");
  }, [lang]);
  return (
    <BrowserRouter>
      <Header />
      <Banners lastDataAt={reference?.fetchedAt ?? null} />
      <Routes>
        <Route path="/" element={<Home reference={reference?.data ?? null} />} />
        <Route path="/new" element={<NewEvent reference={reference?.data ?? null} />} />
        <Route path="/o/:secret" element={<OrganizerPage reference={reference?.data ?? null} />} />
        <Route path="/e/:id" element={<EventPage reference={reference?.data ?? null} mode="public" />} />
        <Route path="/j/:secret" element={<EventPage reference={reference?.data ?? null} mode="join" />} />
        <Route path="/p/:secret" element={<ParticipantPage reference={reference?.data ?? null} />} />
        {/* Sans redirection : l'adresse reste intacte pour les routes des stories suivantes. */}
        <Route path="*" element={<Home reference={reference?.data ?? null} />} />
      </Routes>
    </BrowserRouter>
  );
}
