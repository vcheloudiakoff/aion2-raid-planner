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
import { OrganizerPending } from "./pages/OrganizerPending";
import { ParticipantPending } from "./pages/ParticipantPending";

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
        <Route path="/" element={<Home />} />
        <Route path="/new" element={<NewEvent reference={reference?.data ?? null} />} />
        {/* Page d'attente, remplacée par la vue organisateur en 2-4. */}
        <Route path="/o/:secret" element={<OrganizerPending />} />
        <Route path="/e/:id" element={<EventPage reference={reference?.data ?? null} mode="public" />} />
        <Route path="/j/:secret" element={<EventPage reference={reference?.data ?? null} mode="join" />} />
        {/* Page provisoire, remplacée par la vue participant en 2-3. */}
        <Route path="/p/:secret" element={<ParticipantPending />} />
        {/* Sans redirection : l'adresse reste intacte pour les routes des stories suivantes. */}
        <Route path="*" element={<Home />} />
      </Routes>
    </BrowserRouter>
  );
}
