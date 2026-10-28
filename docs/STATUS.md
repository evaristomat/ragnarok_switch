# Estado atual — 0.1.0-pre12

Atualização: 2026-10-28. Edição experimental em teste manual.

## Integrado

- Rate 15x, SP de habilidades em zero, classes clássicas e limite Base 99.
- Campanha original de 33 etapas e orientação de objetivos/rotas, com reconhecimento do Job Changer.
- Bots, comerciantes, progressão acompanhando o jogador e companheiros temporários.
- HUD para controle, viagens e Asa de Mosca Infinita.
- Atlas de atores e distribuição de carregamentos/atualizações para reduzir trabalho por frame.
- Tradução de rótulos de NPCs e revisão adicional do Campo dos Aprendizes.

## Verificado anteriormente no desenvolvimento

Testes focados de cliente, carregamento de scripts sem erros no cenário final da pre12 e testes gráficos curtos no PC. Há evidências locais não distribuídas; elas não são substituídas por afirmações de certificação na documentação pública.

A pre12 foi compilada, mas não instalada no console durante a preparação deste repositório. A última instalação física verificada é pre8. Não confunda versão compilada com versão instalada.

## Pendências conhecidas

- Testar a campanha completa, incluindo progressão e balanceamento de todos os perfis.
- Confirmar no Switch a fluidez e a resolução dos travamentos relatados em Comodo; a causa original ainda não foi confirmada por log do aparelho.
- Revisar demais diálogos antigos; a tradução não foi integralmente revisada por uma pessoa.
- Validar compilação integral em máquina limpa. As receitas de preparação foram organizadas; a imagem devkitPro e alguns pacotes não estão congelados por digest.
- Completar um importador público de overlays clássicos/PT-BR a partir de dados próprios do usuário. O clone não fornece o pacote comercial nem promete funcionamento com apenas um GRF moderno.
- Resolver a questão de permissão dos defaults AzzyAI antes da publicação pública.

## Identificação do binário local

NRO pre12: 149062785 bytes. SHA256:

~~~text
d496a0461e94326f07edb9d524982fb4bcf7058ed527dc2b33d77724ebed1c22
~~~

Esse hash identifica o build local anterior à organização. A tarefa de organização não recompila nem altera esse NRO. O snapshot de fontes para GitHub contém melhorias nos scripts de preparação e não é uma declaração de que todos os arquivos do snapshot correspondem byte a byte ao build anterior.
