# Índice de ferramentas

Execute os scripts na raiz do projeto. As novas ferramentas usam caminhos relativos ao checkout; scripts históricos do mantenedor podem exigir E: ou evidências locais e não fazem parte do fluxo público.

## Fluxo público

| Ferramenta | Uso |
| --- | --- |
| setup.ps1 | Preparar imagens, NVK e banco inicial; verificar pré-requisitos |
| setup_seed.py | Criar banco inicial a partir do SQL sem sobrescrever outro banco |
| build.ps1 | Compilar host, servidores Switch e cliente/NRO |
| build_host.sh, build_switch_servers.sh, build_client.sh | Estágios internos executados nos containers |
| build_nvk.sh | Compilar a cópia vendorizada do driver, sem depender de Git aninhado |
| package_client.sh | Empacotar ELF, metadados e RomFS em NRO |
| test.ps1, prepare_tests.sh | Testes de bibliotecas, mundo e gráficos |
| github/check_repository.py | Verificar documentação, SQL inicial e seleção pública |
| github/test_packaging.py | Testar exclusões, recusa de sobrescrita e criação do banco |
| github/export_public.py | Gerar cópia selecionada com histórico novo |

## Conteúdo e testes específicos

- generate_campaign.py e generate_classic_travel.py: gerar campanha e catálogo de destinos; saídas atuais estão versionadas.
- configure_classic_population.py: configuração de população; revisar o diff antes de usar.
- test_world.py e test_graphics.py: integração em mundos descartáveis no container.
- test_campaign*.py, verify_campaign*.py e test_population_progression.py: cenários específicos; podem exigir fixtures e evidências locais.
- test_comodo_memory.py: medição local comparativa; não certifica o desempenho do Switch.

## Histórico local preservado

Os instaladores install_pre*.py/install_switch.py, migrações port_renewal_pre2.py/prepare_pre*.py, empacotamento privado pack_assets.py, geradores antigos de tradução e auditorias dependentes de relatórios antigos são preservados no checkout de desenvolvimento. Eles são omitidos do snapshot público conforme github/policy.json; não são requisitos de compilação.

Não mova esses scripts indiscriminadamente: vários calculam caminhos a partir da própria localização ou importam outros scripts. A seleção pública separa o fluxo mantido sem quebrar as ferramentas locais existentes.
