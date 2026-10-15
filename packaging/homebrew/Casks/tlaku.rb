# Cask de Tlaku para el tap propio jahziel1/homebrew-tlaku (esta carpeta es la raíz del tap).
# Instalar:  brew install --cask jahziel1/tlaku/tlaku
# Guía de envío y por qué NO va al cask oficial: docs/publicar.md#homebrew-macos
cask "tlaku" do
  version "0.1.0"
  sha256 "7dbabd44861389c01b3534ea377f69a5403e37e1a9d114dcdb77b6d097f885e6"

  url "https://github.com/jahziel1/tlaku/releases/download/v#{version}/Tlaku_#{version}_universal.dmg"
  name "Tlaku"
  desc "Free software to prepare and send jobs to laser cutters and engravers"
  homepage "https://jahziel1.github.io/tlaku/"

  livecheck do
    url :url
    strategy :github_latest
  end

  # La app trae su propio actualizador (Ayuda > Buscar actualizaciones).
  auto_updates true
  depends_on macos: :big_sur

  app "Tlaku.app"

  zap trash: [
    "~/Library/Application Support/io.github.jahziel1.tlaku",
    "~/Library/Caches/io.github.jahziel1.tlaku",
    "~/Library/HTTPStorages/io.github.jahziel1.tlaku",
    "~/Library/Logs/io.github.jahziel1.tlaku",
    "~/Library/Preferences/io.github.jahziel1.tlaku.plist",
    "~/Library/Saved Application State/io.github.jahziel1.tlaku.savedState",
    "~/Library/WebKit/io.github.jahziel1.tlaku",
  ]

  caveats <<~EOS
    Tlaku (beta) todavía no está firmado ni notarizado por Apple, así que Gatekeeper
    bloquea la primera apertura (en Apple Silicon puede decir que "está dañado").
    Una vez instalada, quita la marca de cuarentena:

      xattr -dr com.apple.quarantine /Applications/Tlaku.app

    o abre Ajustes del Sistema > Privacidad y seguridad y pulsa "Abrir de todos modos".
    Ninguna función de máquina se ha probado con muchos láseres reales: usa potencia baja,
    material de desecho y la mano cerca del paro de emergencia.
  EOS
end
