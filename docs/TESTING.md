# Testes e relatórios

## Verificação leve do repositório

~~~powershell
python scripts/github/check_repository.py
python scripts/github/test_packaging.py
~~~

A exportação pública acrescenta a opção --public para verificar arquivos proibidos, tamanho, padrões de segredos conhecidos e caminhos pessoais. Essa verificação não certifica licenças, ausência absoluta de segredos nem execução do jogo.

## Testes de código e integração

Após preparar imagens e banco inicial, consulte [BUILDING.md](BUILDING.md):

~~~powershell
./scripts/test.ps1 -Stage units
./scripts/build.ps1 -Stage host
./scripts/test.ps1 -Stage world
./scripts/test.ps1 -Stage graphics
~~~

O estágio units testa bibliotecas do cliente e prepara a ferramenta de protocolo. world exige servidores host e o probe compilados. graphics usa servidor host, cliente desktop, Xvfb e Vulkan de software. Os cenários usam mundos descartáveis; não use seu save principal como fixture.

Testes adicionais de campanha, migração, recompensas e população estão listados em scripts/README.md. Alguns cenários históricos dependem de evidências/recursos locais omitidos do clone público; não são parte da verificação leve de CI.

## Campanha e desempenho

A campanha não foi verificada integralmente do início ao fim. Testes no PC não certificam o uso no console. As otimizações de pre10/pre11 tiveram verificações locais, mas o travamento relatado em Comodo não possui causa confirmada por log do aparelho.

Não marque uma versão como estável com base apenas no build ou em uma captura. Registre mapa, duração, quantidade de atores, personagem, companheiros, equipamento e versão; compare o mesmo cenário antes/depois. Mudanças de memória, alocações de GPU e FPS são métricas diferentes.

## Ao reportar um bug

Use o modelo de issue. Informe passos, resultado esperado, resultado observado e se ocorre novamente após reiniciar. Anexe logs revisados; remova contas, caminhos pessoais e outros dados privados. Não publique seu banco de personagens como parte do repositório.

A CI fornecida valida estrutura, SQL inicial e empacotamento. Não compila o NRO nem atesta jogabilidade no Switch.
