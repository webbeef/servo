// SPDX-License-Identifier: AGPL-3.0-or-later

// Uses fixed location since Geolocation

const OPEN_METEO_URL = "https://api.open-meteo.com/v1/forecast";

// Fixed location: Tristan da Cunha (37°4′3″S 12°18′40″W)
const FIXED_LAT = -37.0675;
const FIXED_LON = -12.3111;
const LOCATION_NAME = "Tristan da Cunha";

async function fetchWeather(lat, lon) {
  const params = new URLSearchParams({
    latitude: lat,
    longitude: lon,
    current: "temperature_2m,weather_code",
    temperature_unit: "celsius",
  });
  const response = await fetch(`${OPEN_METEO_URL}?${params}`);
  return response.json();
}

function getWeatherIcon(code) {
  // WMO weather codes to Lucide icons
  if (code === 0) return "sun";
  if (code <= 3) return "cloud-sun";
  if (code <= 48) return "cloud-fog";
  if (code <= 67) return "cloud-rain";
  if (code <= 77) return "cloud-snow";
  if (code <= 99) return "cloud-lightning";
  return "cloud";
}

function getWeatherCondition(code) {
  if (code === 0) return "Clear sky";
  if (code <= 3) return "Partly cloudy";
  if (code <= 48) return "Foggy";
  if (code <= 67) return "Rainy";
  if (code <= 77) return "Snowy";
  if (code <= 99) return "Thunderstorm";
  return "Unknown";
}

async function init() {
  try {
    const data = await fetchWeather(FIXED_LAT, FIXED_LON);
    const temp = Math.round(data.current.temperature_2m);
    const code = data.current.weather_code;

    document.getElementById("temperature").textContent = `${temp}°C`;
    document.getElementById("condition").textContent =
      getWeatherCondition(code);
    document.getElementById("location").textContent = LOCATION_NAME;
    document.getElementById(
      "weather-icon"
    ).innerHTML = `<lucide-icon name="${getWeatherIcon(code)}"></lucide-icon>`;
  } catch (e) {
    console.error("[Weather] Failed to fetch weather:", e);
    document.getElementById("condition").textContent = "Weather unavailable";
  }
}

init();
