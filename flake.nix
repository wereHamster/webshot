{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        fontsConf = pkgs.makeFontsConf {
          fontDirectories = with pkgs; [
            liberation_ttf
            dejavu_fonts
            noto-fonts
            noto-fonts-color-emoji
            noto-fonts-cjk-sans
            noto-fonts-cjk-serif
          ];
          includes = [
            ./nix/10-webshot.conf
          ];
        };

        headlessShell = pkgs.runCommand "chromium-headless-shell" { } ''
          mkdir -p $out/bin
          ln -s ${
            pkgs.playwright-driver.browsers.override {
              withChromium = false;
              withFirefox = false;
              withWebkit = false;
              withFfmpeg = false;
              withChromiumHeadlessShell = true;
            }
          }/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell $out/bin/chromium
        '';

        webshotPackage = rustPlatform.buildRustPackage {
          pname = "webshot";
          version = "0.1.0";

          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            pkgs.makeWrapper
          ];

          doCheck = false;

          postInstall = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            wrapProgram $out/bin/webshot \
              --set CHROME "${headlessShell}/bin/chromium" \
              --set FONTCONFIG_FILE "${fontsConf}"
          '';
        };
      in
      {
        packages.default = webshotPackage;

        packages.docker = pkgs.dockerTools.buildLayeredImage {
          name = "webshot";

          contents = [
            webshotPackage
            pkgs.cacert
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            headlessShell
          ];

          config = {
            Cmd = [ "${webshotPackage}/bin/webshot" ];
            ExposedPorts = {
              "3000/tcp" = { };
            };
            Env = [
              "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              "CHROME=${headlessShell}/bin/chromium"
              "FONTCONFIG_FILE=${fontsConf}"
            ];
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            rustToolchain
            pkgs.google-cloud-sdk
            pkgs.skopeo
          ];
        };

        devShells.workflow = pkgs.mkShell {
          nativeBuildInputs = [
            rustToolchain
          ];
        };
      }
    );
}
