{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    nix-develop.url = "github:nicknovitski/nix-develop";
    nix-develop.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      nixpkgs,
      nix-develop,
      ...
    }:
    let
      forAllSystems =
        function: nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed (system: function nixpkgs.legacyPackages.${system});

    in
    {
      packages = forAllSystems (
        pkgs:
        let
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

          webshotPackage = pkgs.rustPlatform.buildRustPackage {
            pname = "webshot";
            version = "0.1.0";

            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            doCheck = false;
          };

          webshotUser = pkgs.runCommand "webshot-user" { } ''
            mkdir -p $out/etc
            echo "webshot:x:1000:1000:webshot:/home/webshot:/bin/nologin" > $out/etc/passwd
            echo "webshot:x:1000:" > $out/etc/group
          '';

        in
        {
          nix-develop = nix-develop.packages.${pkgs.stdenv.hostPlatform.system}.default;

          default = webshotPackage;

          docker = pkgs.dockerTools.buildLayeredImage {
            name = "webshot";

            contents = [
              webshotUser
              webshotPackage
              pkgs.cacert
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.chromium
            ];

            fakeRootCommands = ''
              mkdir -p ./tmp
              chmod 1777 ./tmp
              mkdir -p ./home/webshot
              chown 1000:1000 ./home/webshot
            '';

            config = {
              User = "1000:1000";
              Cmd = [ "${webshotPackage}/bin/webshot" ];
              ExposedPorts = {
                "3000/tcp" = { };
              };
              Env = [
                "HOME=/home/webshot"
                "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
                "CHROME=${pkgs.chromium}/bin/chromium"
                "FONTCONFIG_FILE=${fontsConf}"
              ];
            };
          };
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rustfmt
            pkgs.google-cloud-sdk
            pkgs.skopeo
          ];
        };

        workflow = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.rustc
            pkgs.cargo
          ];
        };
      });
    };
}
