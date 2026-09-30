# Disposable NixOS guest for Faber's privileged Docker workflow.
#
# Faber needs a privileged container with the host cgroup tree, which is
# root-equivalent on whatever kernel it runs on. This guest gives it a kernel
# of its own, so a sandbox regression, fork bomb, OOM or cgroup mistake stops
# at the VM boundary. Built and driven by scripts/vm.sh; nixpkgs is the
# revision pinned in flake.lock.
{
  system ? builtins.currentSystem,
}:
let
  lock = builtins.fromJSON (builtins.readFile ../flake.lock);
  nixpkgs = builtins.fetchTree lock.nodes.nixpkgs.locked;

  guest =
    { pkgs, modulesPath, ... }:
    {
      imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

      networking.hostName = "faber-vm";
      system.stateVersion = "25.05";
      documentation.enable = false;

      virtualisation = {
        graphics = false;
        # Defaults; scripts/vm.sh overrides both through QEMU_OPTS.
        memorySize = 8192;
        cores = 4;
        # Sparse. Holds the Docker image, Cargo registry and target volumes.
        diskSize = 40 * 1024;

        docker.enable = true;

        qemu.options = [
          # The repository, exported read-only by QEMU itself so that guest
          # root cannot write to it whatever it does to its own mount.
          ''-virtfs local,path="$FABER_VM_SRC",security_model=none,mount_tag=fabersrc,readonly=on''
          "-sandbox on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny"
        ];
        fileSystems."/faber-src" = {
          device = "fabersrc";
          fsType = "9p";
          options = [
            "trans=virtio"
            "version=9p2000.L"
            "msize=1048576"
            "ro"
          ];
        };
      };

      # `scripts/vm.sh shell` boots without a job and lands here.
      services.getty.autologinUser = "root";
      environment.systemPackages = with pkgs; [
        curl
        jq
      ];

      # scripts/vm.sh hands over one job through the shared directory. Its
      # output and exit status go back the same way, then the guest powers
      # off. Creating `stop` in the shared directory ends the run early.
      systemd.services.faber-job = {
        description = "Run the job handed over by scripts/vm.sh";
        wantedBy = [ "multi-user.target" ];
        requires = [ "docker.service" ];
        wants = [ "network-online.target" ];
        after = [
          "docker.service"
          "network-online.target"
        ];
        unitConfig = {
          ConditionPathExists = "/tmp/shared/job.sh";
          RequiresMountsFor = "/tmp/shared /faber-src";
        };
        path = with pkgs; [
          bash
          coreutils
          curl
          docker
          findutils
          gawk
          gnugrep
          gnused
          jq
          kmod
          procps
          util-linux
        ];
        serviceConfig = {
          Type = "oneshot";
          TimeoutStartSec = "infinity";
          WorkingDirectory = "/faber-src";
        };
        script = ''
          shared=/tmp/shared
          (
            until [ -e "$shared/stop" ]; do sleep 1; done
            systemctl poweroff --no-block
          ) &

          status=0
          bash "$shared/job.sh" >>"$shared/output.log" 2>&1 || status=$?
          echo "$status" >"$shared/exit-code"
          systemctl poweroff --no-block
        '';
      };
    };

  nixos = import "${nixpkgs}/nixos" {
    inherit system;
    configuration = guest;
  };
in
nixos.config.system.build.vm
