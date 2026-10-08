# Plume

Fenêtre native et légère pour l'**application web officielle de Discord**, sous Windows.
Écrite en Rust, elle affiche https://discord.com dans **WebView2**, le moteur Chromium intégré à
Windows, au lieu d'embarquer son propre navigateur comme le fait Discord desktop (Electron).

Plume n'est pas affilié à Discord. La page n'est jamais modifiée : aucun script injecté, aucun mod.

## Gains attendus (estimations à confirmer par vos propres mesures)

| | Discord desktop | Plume |
|---|---|---|
| Espace disque | ~1 Go | un seul `.exe` de quelques Mo (WebView2 est déjà dans Windows) |
| Mémoire au quotidien | ~1 Go, davantage sur de longues sessions | ~350 à 500 Mo |
| Fenêtre réduite | — | WebView2 passe en mode « mémoire basse » |
| Démarrage | écran de chargement + mises à jour | direct |

L'essentiel de la mémoire restante est consommé par l'application web de Discord elle-même.

### Ce que Plume fait pour consommer le moins possible

- **Fenêtre cachée ou réduite** : la page n'est plus dessinée (ni processeur ni carte graphique
  dépensés pour rien) et WebView2 passe en mode mémoire basse. Messages, notifications et appels
  continuent.
- **Composants inutiles désactivés** : lecteur PDF et interface interne d'Edge.
- **Mode éco, optionnel** : le mode « appareil peu puissant » de Chromium (caches réduits, rendu plus
  économe, qualité visuelle parfois un peu moindre). À cocher dans le menu de l'icône (voir plus bas).
- **Jamais au détriment de la sécurité** : isolation des sites, bac à sable et SmartScreen restent
  actifs (un test l'impose).

### Ce que vous pouvez régler dans Discord (gains souvent les plus importants)

Paramètres utilisateur de Discord :
- **Accessibilité** → activer « Réduire les animations » ; désactiver la lecture automatique des GIF,
  les émojis animés et les stickers animés.
- **Texte et images** → désactiver l'aperçu des liens si vous n'en avez pas besoin.
- Quitter les serveurs très actifs que vous ne lisez plus : chaque serveur ouvert coûte de la mémoire.

## Ce qui fonctionne, et les limites

Messages, appels vocaux et vidéo, partage d'écran, notifications, glisser-déposer de fichiers.

Comme Discord, **la croix ne quitte pas** : Plume se cache dans la zone de notification (près de
l'horloge) et continue de recevoir messages, notifications et appels. Un clic gauche sur l'icône
rouvre la fenêtre ; clic droit → « Quitter » pour fermer vraiment. Pour que l'icône reste toujours
visible : Paramètres Windows → Personnalisation → Barre des tâches → Autres icônes → activer Plume.

**Menu de l'icône (clic droit)** : « Ouvrir Plume », **Mode éco**, **Mode éphémère**, « Redémarrer
Plume pour appliquer », « Quitter ». Les deux modes sont mémorisés ; ils prennent effet au prochain
démarrage de Plume, jamais en plein appel (le redémarrage n'est proposé, pas imposé). Les options
`--eco` et `--ephemeral` en ligne de commande restent possibles et forcent le mode.

**Badge de mentions** : comme Discord, un badge rouge avec le nombre de mentions non lues apparaît sur
l'icône de la barre des tâches et sur celle de la zone de notification. Plume lit seulement le titre de
la page, où Discord indique déjà ce nombre.

**Robustesse** : si le moteur de la page plante, elle est rechargée ; si WebView2 entier s'arrête, Plume
redémarre. Sans réseau au lancement, Plume réessaie tout seul (5 s, 10 s, 20 s, 40 s, puis chaque
minute). Une page qui ne répond plus n'est volontairement *pas* rechargée : pendant une partie, elle
peut simplement manquer de processeur, et la recharger couperait l'appel.

**Fenêtre** : taille, position et état agrandi sont mémorisés (si l'écran a été débranché, Plume
revient à la position par défaut).

**Pour jouer** : en partie, c'est le jeu qui a le focus, donc le push-to-talk de Discord ne réagit
pas. Utilisez la **détection de la voix** (Paramètres Discord → Voix et vidéo → Mode d'entrée).
Fenêtre cachée ou réduite, Plume ne dessine plus rien : le jeu récupère processeur et carte graphique,
l'appel continue.

Limites (ce sont celles de Discord dans un navigateur) :
- le **push-to-talk** et les raccourcis ne marchent que lorsque la fenêtre est active ;
- pas de détection des jeux ni d'overlay ;
- les liaisons de comptes (Spotify, Twitch…) et les paiements s'ouvrent dans votre navigateur.

## Sécurité

| Risque | Protection |
|---|---|
| Une page piégée appelle du code de votre PC | Aucun pont page ↔ système : ni IPC, ni objets hôtes, ni messages web |
| Hameçonnage dans une fenêtre sans barre d'adresse | Seul `https://discord.com` (domaine exact) s'affiche dans Plume ; tout autre site s'ouvre dans votre navigateur, où l'adresse est visible |
| Pop-up qui remplace Discord (fausse page de consentement OAuth…) | Les pop-ups ne s'ouvrent jamais dans Plume, uniquement dans le navigateur (une par seconde au maximum) |
| Lancement d'un programme via un lien (`ms-msdt:`, `search-ms:`, `file:`…) | Seuls `http` et `https` sont transmis au navigateur, tout le reste est bloqué |
| Accès au micro, à la caméra… par un contenu intégré | Micro, caméra, notifications et lecture automatique accordés **uniquement** à `https://discord.com` ; tout le reste (géolocalisation, lecture du presse-papiers, fichiers…) est refusé ; décisions jamais mémorisées |
| Téléchargement piégé | Seuls les fichiers des serveurs de Discord sont acceptés, et SmartScreen les vérifie |
| Vol d'identifiants enregistrés | Enregistrement des mots de passe et remplissage automatique désactivés ; outils de développement désactivés |
| DLL piégée à côté de l'exécutable | Les DLL dont dépend Plume ne sont cherchées que dans `System32`, jamais dans le dossier courant ni dans le `PATH` |
| Faille du moteur web | WebView2 est maintenu et mis à jour automatiquement par Microsoft, souvent plus vite que le Chromium figé d'une application Electron |
| Chaîne d'approvisionnement | 78 crates, versions figées (`Cargo.lock`), auditées avec `cargo audit` ; vous compilez vous-même |

**Vos données** se trouvent dans `%LOCALAPPDATA%\Plume`, accessible uniquement à votre compte Windows.
Supprimer ce dossier vous déconnecte complètement.

**Limite incontournable :** comme pour Discord desktop ou un navigateur, un logiciel malveillant qui
tourne déjà sous votre session Windows peut lire la session enregistrée. Si vous voulez qu'il n'y ait
rien à voler, lancez Plume en **mode éphémère** : rien n'est écrit sur le disque, et vous vous
reconnectez à chaque lancement.

```powershell
.\plume.exe --ephemeral
```

**Recommandations :** activez la double authentification sur votre compte Discord (idéalement avec une
clé d'accès, ou *passkey*), gardez Windows à jour, et rangez `plume.exe` dans son propre dossier.

## Recevoir `plume.exe` sans rien installer (GitHub Actions)

Le fichier `.github/workflows/build.yml` fait compiler Plume par les serveurs Windows de GitHub. Votre
PC n'a alors besoin ni de Rust ni des outils de Visual Studio (plusieurs Go).

1. Créez un dépôt **privé** sur GitHub (par exemple `plume`) et envoyez-y ce dossier.
2. À chaque envoi, l'onglet **Actions** vérifie le code (format, lint, tests, audit des dépendances) et
   compile ; `plume.exe` et son empreinte SHA-256 sont téléchargeables dans « Artifacts ».
3. Pour une version : créez une étiquette `v…` (par exemple `v0.2.0`). Une **Release** est publiée
   avec `plume.exe` et son empreinte SHA-256.

Vérifier l'exécutable téléchargé en comparant son empreinte avec le fichier `plume.exe.sha256` publié
à côté (si le dépôt devient public, GitHub ajoute aussi une attestation de provenance, vérifiable avec
`gh attestation verify plume.exe --repo DonRosio/plume`) :

```powershell
(Get-FileHash plume.exe -Algorithm SHA256).Hash
```

Windows affichera une fois l'avertissement SmartScreen (fichier téléchargé, non signé) : « Informations
complémentaires » → « Exécuter quand même ». Les actions GitHub utilisées sont épinglées sur une
empreinte de commit précise, pour qu'une étiquette détournée ne puisse pas changer ce qui s'exécute.

## Compiler sous Windows

Prérequis :
1. Windows 10 ou 11 avec le **Runtime WebView2** (déjà présent sur Windows 11 ; sinon, installez le
   « Evergreen Runtime » depuis le site de Microsoft).
2. **Rust** : téléchargez `rustup-init.exe` sur https://rustup.rs et lancez-le. S'il le propose,
   acceptez l'installation des outils C++ de Visual Studio (nécessaires pour l'édition de liens).
   Gardez la chaîne d'outils par défaut (`msvc`) : elle produit un `.exe` autonome.

Puis, dans un terminal ouvert dans le dossier `plume` :

```powershell
cargo build --release
```

L'exécutable est `target\release\plume.exe`, avec l'icône de plume intégrée. Compilez depuis un
dossier personnel (par exemple `Documents\plume`) : Cargo doit pouvoir écrire dans `target\`.

Installation : copiez **uniquement** `plume.exe` dans `C:\Program Files\Plume\` (droits
administrateur demandés une fois ; ce dossier n'est pas modifiable par un programme ordinaire, ce qui
protège l'exécutable). Lancez-le, puis clic droit sur son icône dans la barre des tâches → « Épingler
à la barre des tâches ».

Facultatif, mais conseillé avant chaque compilation : vérifiez les dépendances.

```powershell
cargo install cargo-audit
```

```powershell
cargo audit
```

## Checklist de test

1. **Démarrage** : fond sombre, puis Discord ; connexion par mot de passe + 2FA, QR code ou clé d'accès.
2. **Relance** : vous êtes toujours connecté ; un second lancement ramène la fenêtre existante.
3. **Fichiers** : envoyer une pièce jointe ; en télécharger une (la barre de téléchargement apparaît).
4. **Appel vocal** : vous entendez et on vous entend.
5. **Partage d'écran.**
6. **Notifications** quand la fenêtre est réduite.
7. **Liens** : un lien vers un autre site s'ouvre dans votre navigateur, pas dans Plume.
8. **Mode éphémère** : avec `--ephemeral`, après fermeture et relance, Discord redemande la connexion.

## Mesurer la mémoire (PowerShell)

À faire après une dizaine de minutes d'utilisation comparable (mêmes serveurs ouverts).

Discord desktop :

```powershell
"{0:N0} Mo" -f ((Get-Process Discord -ErrorAction SilentlyContinue | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB)
```

Plume (le processus `plume.exe` et ses processus WebView2) :

```powershell
$ids = @((Get-Process plume).Id) + @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*\Plume\*' } | ForEach-Object ProcessId); "{0:N0} Mo" -f ((Get-Process -Id $ids | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB)
```

## Mises à jour

WebView2 se met à jour avec Windows, et l'application web de Discord à chaque chargement. Pour Plume
lui-même, récupérez la nouvelle Release GitHub (ou recompilez après `cargo audit`). Il n'y a
volontairement aucune mise à jour automatique, donc aucun canal de mise à jour à pirater.

## Démarche

Ce projet est né d'un besoin concret : réduire la mémoire consommée par Discord sur mon PC de jeu,
pendant mes parties de League of Legends en vocal avec mes amis.

Je l'ai conçu et piloté, et je l'ai développé avec l'assistance de **Claude** (Anthropic), utilisé
comme binôme de programmation. Mon rôle :

- **Définir le besoin et les exigences** : une alternative plus légère que Discord desktop, utilisable
  avec mon compte existant, et le plus sécurisée possible.
- **Arbitrer les choix d'architecture.** J'ai d'abord exploré un client entièrement natif, écarté parce
  qu'il aurait enfreint les conditions d'utilisation de Discord et exposé mon compte. J'ai comparé
  ensuite plusieurs voies (navigateur, client tiers existant, fenêtre WebView2) avant de retenir
  WebView2 pour Windows, qui garde les appels vocaux.
- **Fixer les compromis** : sécurité jamais sacrifiée à la performance, mode éco optionnel plutôt
  qu'imposé, pas de démarrage automatique avec Windows, pas de rechargement automatique d'une page
  qui ne répond plus (pour ne pas couper un appel en pleine partie).
- **Valider sur ma machine Windows** : compilation, fonctionnement, mesures de consommation.

Le code a été écrit avec Claude, puis vérifié : tests unitaires des règles de sécurité, analyse
statique stricte (clippy), audit des dépendances, compilation et publication automatiques sur GitHub.
Les principales failles relevées pendant les relectures ont été corrigées : pop-ups capables de
remplacer la fenêtre, SmartScreen désactivé par défaut dans la bibliothèque wry, DLL piégées.

## Code

| Fichier | Rôle |
|---|---|
| `src/policy.rs` | Toutes les règles de sécurité (navigation, pop-ups, permissions, téléchargements), testées unitairement |
| `src/app.rs` | Fenêtre, durcissement de WebView2, gestionnaires d'événements, menu de l'icône |
| `src/badge.rs` | Lecture du nombre de mentions dans le titre et dessin du badge |
| `src/settings.rs` | Réglages et position de la fenêtre (`%LOCALAPPDATA%\Plume\settings.txt`) |
| `.github/workflows/build.yml` | Compilation et vérifications automatiques sur GitHub |
| `src/main.rs` | Point d'entrée |
| `build.rs`, `assets/plume.ico` | Icône et nom du programme intégrés à `plume.exe` |

Tests (sous n'importe quel système) :

```bash
cargo test
```

Vérification de la compilation pour Windows depuis Linux :

```bash
cargo clippy --target x86_64-pc-windows-msvc -- -D warnings
```
