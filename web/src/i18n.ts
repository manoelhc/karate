import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from './locales/en.json';
import pt from './locales/pt-BR.json';
export function detectedLocale(): 'en' | 'pt-BR' {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem('karate.locale');
  } catch {
    /* Optional preference storage. */
  }
  return saved === 'en' || saved === 'pt-BR'
    ? saved
    : navigator.language.toLowerCase().startsWith('pt')
      ? 'pt-BR'
      : 'en';
}
export function localeDocument(language: string): void {
  document.documentElement.lang = language;
  document.documentElement.dir = 'ltr';
  try {
    localStorage.setItem('karate.locale', language);
  } catch {
    /* No storage required to sign in. */
  }
}
void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, 'pt-BR': { translation: pt } },
  lng: detectedLocale(),
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  initAsync: false,
});
i18n.on('languageChanged', localeDocument);
localeDocument(i18n.language);
export default i18n;
