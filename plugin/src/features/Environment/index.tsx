import React, { useContext, useState } from 'react'
import DevnetAccountSelector from '../../components/DevnetAccountSelector'
import './styles.css'
import {
  type ConnectOptions,
  type DisconnectOptions,
  type StarknetWindowObject,
  connect,
  disconnect
} from 'get-starknet'
import { RemixClientContext } from '../../contexts/RemixClientContext'
import { type Devnet, devnets } from '../../utils/network'
import EnvironmentSelector from '../../components/EnvironmentSelector'
import { ConnectionContext } from '../../contexts/ConnectionContext'
import Wallet from '../../components/Wallet'
import { EnvCard } from '../../components/EnvCard'
import { RxDotFilled } from 'react-icons/rx'

// eslint-disable-next-line @typescript-eslint/no-empty-interface
interface EnvironmentProps {}

const Environment: React.FC<EnvironmentProps> = () => {
  const remixClient = useContext(RemixClientContext)
  const { setAccount, setProvider } = useContext(ConnectionContext)

  // START: DEVNET
  const [devnet, setDevnet] = useState<Devnet>(devnets[0])
  const [env, setEnv] = useState<string>('devnet')
  const [isDevnetAlive, setIsDevnetAlive] = useState<boolean>(true)
  // END: DEVNET

  // START: WALLET
  const [starknetWindowObject, setStarknetWindowObject] =
    useState<StarknetWindowObject | null>(null)

  // eslint-disable-next-line @typescript-eslint/explicit-function-return-type
  const connectWalletHandler = async (
    options: ConnectOptions = {
      modalMode: 'alwaysAsk',
      modalTheme: 'dark'
    }
  ) => {
    try {
      const connectedStarknetWindowObject = await connect(options)
      if (connectedStarknetWindowObject == null) {
        throw new Error('Failed to connect to wallet')
      }
      await connectedStarknetWindowObject.enable({ starknetVersion: 'v4' })
      connectedStarknetWindowObject.on(
        'accountsChanged',
        (accounts: string[]) => {
          console.log('accountsChanged', accounts)
          void connectWalletHandler({
            modalMode: 'neverAsk',
            modalTheme: 'dark'
          })
          connectedStarknetWindowObject.off(
            'accountsChanged',
            (_accounts: string[]) => {}
          )
        }
      )

      connectedStarknetWindowObject.on('networkChanged', (network?: string) => {
        console.log('networkChanged', network)
        void connectWalletHandler({
          modalMode: 'neverAsk',
          modalTheme: 'dark'
        })
        connectedStarknetWindowObject.off(
          'networkChanged',
          (_network?: string) => {}
        )
      })
      setStarknetWindowObject(connectedStarknetWindowObject)
      if (connectedStarknetWindowObject.account != null) {
        setAccount(connectedStarknetWindowObject.account)
      }
      if (connectedStarknetWindowObject.provider != null) {
        setProvider(connectedStarknetWindowObject.provider)
      }
    } catch (e) {
      if (e instanceof Error) {
        await remixClient.call('notification' as any, 'alert', e)
      }
      setStarknetWindowObject(null)
      console.log(e)
    }
  }

  const disconnectWalletHandler = async (
    options: DisconnectOptions = {
      clearLastWallet: true
    }
  ): Promise<void> => {
    if (starknetWindowObject != null) {
      starknetWindowObject.off('accountsChanged', (_accounts: string[]) => {})
      starknetWindowObject.off('networkChanged', (_network?: string) => {})
    }
    await disconnect(options)
    setStarknetWindowObject(null)
    setAccount(null)
    setProvider(null)
  }

  // END: WALLET

  return (
    <div className="starknet-connection-component mb-8">
      <EnvCard
        header="Environment"
        setEnv={setEnv}
        disconnectWalletHandler={disconnectWalletHandler}
      >
          <>
            <div className="flex">
              <label className="">Environment selection</label>
              <div className='flex_dot'>
              <EnvironmentSelector
                env={env}
                setEnv={setEnv}
                devnet={devnet}
                setDevnet={setDevnet}
                connectWalletHandler={connectWalletHandler}
                disconnectWalletHandler={disconnectWalletHandler}
              />
              {isDevnetAlive ? <RxDotFilled size={'30px'} color="lime" title='Devnet is live'/> : <RxDotFilled size={'30px'} color="red" title='Devnet server down'/>}
              </div>
            </div>
            <div className="flex">
              {env === 'devnet'
                ? (
                <DevnetAccountSelector devnet={devnet} isDevnetAlive={isDevnetAlive} setIsDevnetAlive={setIsDevnetAlive} />
                  )
                : (
                <Wallet
                  starknetWindowObject={starknetWindowObject}
                  connectWalletHandler={() => connectWalletHandler}
                  disconnectWalletHandler={() => disconnectWalletHandler}
                />
                  )}
            </div>
          </>
      </EnvCard>
    </div>
  )
}

export { Environment }
